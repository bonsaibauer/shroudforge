#include "graphics/validate_overlay_shader.h"
#include "abi/map_overlay_api.h"
#include <spirv-tools/libspirv.h>
#include <spirv/unified1/spirv.hpp>
#include <algorithm>
#include <cstring>
#include <map>
#include <set>
#include <string_view>
#include <vector>

namespace OverlayRuntime {
namespace {
struct Node { unsigned op{};std::vector<unsigned> args; };
using Decorations=std::map<unsigned,std::vector<unsigned>>;
struct Interface {
    std::vector<Node> nodes;
    std::vector<Decorations> decorations;
    std::map<std::pair<unsigned,unsigned>,Decorations> members;
    unsigned value(unsigned id,unsigned decoration,unsigned fallback=UINT32_MAX) const {
        const auto found=decorations[id].find(decoration);
        return found!=decorations[id].end() && found->second.size()==1 ? found->second[0]:fallback;
    }
    bool scalar(unsigned id,unsigned op) const {
        const auto& n=nodes[id];return n.op==op && !n.args.empty() && n.args[0]==32;
    }
    bool vector(unsigned id,unsigned count) const {
        const auto& n=nodes[id];return n.op==spv::OpTypeVector && n.args.size()==2 &&
            n.args[1]==count && scalar(n.args[0],spv::OpTypeFloat);
    }
    unsigned extent(unsigned id,unsigned depth=0,unsigned stride=0,bool row=false) {
        if(depth>16)return UINT32_MAX;
        const auto& n=nodes[id];
        if(scalar(id,spv::OpTypeFloat) || scalar(id,spv::OpTypeInt))return 4;
        if(n.op==spv::OpTypeVector && n.args.size()==2 && n.args[1]>=2 && n.args[1]<=4)
            return extent(n.args[0],depth+1)==4 ? n.args[1]*4:UINT32_MAX;
        if(n.op==spv::OpTypeMatrix && n.args.size()==2 && stride && stride<=128){
            const auto& column=nodes[n.args[0]];
            if(column.op!=spv::OpTypeVector || column.args.size()!=2 || extent(n.args[0],depth+1)==UINT32_MAX)return UINT32_MAX;
            const auto count=row?column.args[1]:n.args[1];return count<=4 && count*stride<=128?count*stride:UINT32_MAX;
        }
        if(n.op==spv::OpTypeArray && n.args.size()==2){
            const auto& count=nodes[n.args[1]];const auto step=value(id,spv::DecorationArrayStride);
            const auto element=extent(n.args[0],depth+1,stride,row);
            if(count.op!=spv::OpConstant || count.args.size()!=2 || !scalar(count.args[0],spv::OpTypeInt) ||
                !count.args[1] || count.args[1]>32 || step==UINT32_MAX || step<element || step>128)return UINT32_MAX;
            return count.args[1]*step<=128?count.args[1]*step:UINT32_MAX;
        }
        if(n.op!=spv::OpTypeStruct)return UINT32_MAX;
        unsigned size=0;
        for(unsigned i=0;i<n.args.size();++i){
            const auto& d=members[{id,i}];
            const auto offset=d.find(spv::DecorationOffset),matrix=d.find(spv::DecorationMatrixStride);
            if(offset==d.end() || offset->second.size()!=1)return UINT32_MAX;
            const auto bytes=extent(n.args[i],depth+1,matrix==d.end()?0:matrix->second[0],d.contains(spv::DecorationRowMajor));
            if(bytes>128 || offset->second[0]>128-bytes)return UINT32_MAX;
            size=(std::max)(size,offset->second[0]+bytes);
        }
        return size;
    }
};
bool string_is(std::span<const unsigned> words,std::string_view expected) {
    const auto* text=reinterpret_cast<const char*>(words.data());const auto size=words.size_bytes();
    return size>expected.size() && !std::memcmp(text,expected.data(),expected.size()) && text[expected.size()]==0;
}
bool reflect(std::span<const unsigned> words) {
    Interface f;f.nodes.resize(words[3]);f.decorations.resize(words[3]);
    unsigned entries=0,pushes=0,outputs=0,entry_id=0;
    std::set<unsigned> interfaces,bindings;
    for(std::size_t at=5;at<words.size();){
        const auto count=words[at]>>16,op=words[at]&65535;const auto a=words.subspan(at+1,count-1);
        switch(op){
        case spv::OpCapability:
            if(a[0]!=spv::CapabilityShader && a[0]!=spv::CapabilityMatrix &&
                a[0]!=spv::CapabilityImageQuery && a[0]!=spv::CapabilityDerivativeControl)return false;
            break;
        case spv::OpExtension:case spv::OpDecorationGroup:case spv::OpGroupDecorate:case spv::OpGroupMemberDecorate:return false;
        case spv::OpExtInstImport:if(!string_is(a.subspan(1),"GLSL.std.450"))return false;break;
        case spv::OpMemoryModel:if(a[0]!=spv::AddressingModelLogical || a[1]!=spv::MemoryModelGLSL450)return false;break;
        case spv::OpEntryPoint:{
            if(++entries!=1 || a[0]!=spv::ExecutionModelFragment || !string_is(a.subspan(2),"main"))return false;
            entry_id=a[1];
            // main plus NUL occupies two words.
            for(std::size_t i=4;i<a.size();++i)interfaces.insert(a[i]);break;
        }
        case spv::OpExecutionMode:if(a[1]!=spv::ExecutionModeOriginUpperLeft)return false;break;
        case spv::OpDecorate:f.decorations[a[0]][a[1]]={a.begin()+2,a.end()};break;
        case spv::OpMemberDecorate:f.members[{a[0],a[1]}][a[2]]={a.begin()+3,a.end()};break;
        case spv::OpTypeFloat:case spv::OpTypeInt:case spv::OpTypeBool:case spv::OpTypeVector:case spv::OpTypeMatrix:
        case spv::OpTypeStruct:case spv::OpTypeArray:case spv::OpTypeImage:case spv::OpTypeSampledImage:case spv::OpTypePointer:
            f.nodes[a[0]]={op,{a.begin()+1,a.end()}};break;
        case spv::OpVariable:case spv::OpConstant:
            f.nodes[a[1]]={op,{a[0]}};f.nodes[a[1]].args.insert(f.nodes[a[1]].args.end(),a.begin()+2,a.end());break;
        default:break;
        }at+=count;
    }
    if(entries!=1 || !entry_id)return false;
    for(unsigned id=1;id<f.nodes.size();++id){
        const auto& n=f.nodes[id];if(n.op!=spv::OpVariable)continue;
        const auto& pointer=f.nodes[n.args[0]];if(pointer.op!=spv::OpTypePointer || pointer.args.size()!=2)return false;
        const auto storage=n.args[1],type=pointer.args[1];
        if(storage==spv::StorageClassFunction || storage==spv::StorageClassPrivate)continue;
        if(storage==spv::StorageClassPushConstant){
            if(++pushes!=1 || f.nodes[type].op!=spv::OpTypeStruct ||
                !f.decorations[type].contains(spv::DecorationBlock) || f.extent(type)>128)return false;
            const auto first=f.members.find({type,0});
            if(first==f.members.end() || !first->second.contains(spv::DecorationOffset) ||
                first->second.at(spv::DecorationOffset)!=std::vector<unsigned>{0})return false;
            continue;
        }
        if(storage==spv::StorageClassUniformConstant){
            const auto binding=f.value(id,spv::DecorationBinding);const auto& sampled=f.nodes[type];
            if(f.value(id,spv::DecorationDescriptorSet)!=0 || binding>=4 || !bindings.insert(binding).second ||
                sampled.op!=spv::OpTypeSampledImage || sampled.args.size()!=1)return false;
            const auto& image=f.nodes[sampled.args[0]];
            if(image.op!=spv::OpTypeImage || image.args.size()!=7 || !f.scalar(image.args[0],spv::OpTypeFloat) ||
                image.args[1]!=spv::Dim2D || image.args[2]!=0 || image.args[3]!=0 || image.args[4]!=0 ||
                image.args[5]!=1 || image.args[6]!=spv::ImageFormatUnknown)return false;
            continue;
        }
        if(!interfaces.contains(id))return false;
        if(storage==spv::StorageClassInput){
            const auto builtin=f.value(id,spv::DecorationBuiltIn);
            if(builtin!=UINT32_MAX){if(builtin!=spv::BuiltInFragCoord || !f.vector(type,4))return false;}
            else if(f.value(id,spv::DecorationLocation)!=0 || f.value(id,spv::DecorationComponent,0)!=0 || !f.vector(type,2))return false;
            for(auto d:{spv::DecorationFlat,spv::DecorationNoPerspective,spv::DecorationCentroid,spv::DecorationSample})
                if(f.decorations[id].contains(d))return false;
        }else if(storage==spv::StorageClassOutput){
            if(++outputs!=1 || f.value(id,spv::DecorationLocation)!=0 || !f.vector(type,4) ||
                f.value(id,spv::DecorationIndex,0)!=0 || f.value(id,spv::DecorationComponent,0)!=0)return false;
        }else return false;
    }
    return outputs==1;
}
}
bool ValidateOverlayFragment(std::span<const std::uint32_t> words) {
    if(words.size()<5 || words.size_bytes()>SF_OVERLAY_MAX_SHADER_BYTES || words[0]!=spv::MagicNumber ||
        words[1]!=0x00010000 || !words[3] || words[3]>65536 || words[4])return false;
    const auto context=spvContextCreate(SPV_ENV_VULKAN_1_0);if(!context)return false;
    struct Release {spv_context context;~Release(){spvContextDestroy(context);}} release{context};
    const auto result=spvValidateBinary(context,words.data(),words.size(),nullptr);
    return result==SPV_SUCCESS && reflect(words);
}
}
