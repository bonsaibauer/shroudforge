#pragma once
#include "abi/map_overlay_api.h"
#include "graphics/validate_overlay_shader.h"
#include <nlohmann/json.hpp>
#include <charconv>
#include <algorithm>
#include <cmath>
#include <limits>
#include <memory>
#include <stdexcept>
#include <vector>

namespace OverlayRuntime {
// Immutable copied input, stored by the existing service Owner. No Lua, borrowed
// caller bytes, game pointers or additional renderer registry reach Dispatch.
struct OwnedOverlayScene {
    struct Texture {
        SFOverlayTextureV1 value{};
        std::vector<std::uint8_t> pixels,coverage;
        void Bind(){value.rgba=pixels.data();value.rgba_bytes=pixels.size();
            value.coverage_mask=coverage.empty()?nullptr:coverage.data();value.coverage_mask_bytes=coverage.size();}
    };
    struct Textured {Texture texture;SFOverlayTextureCommandV2 command{};};
    struct Program {
        SFOverlayProgramCommandV1 command{};
        std::vector<std::uint32_t> words;
        std::vector<std::uint8_t> constants;
        std::vector<Texture> textures;
        std::vector<SFOverlayTextureV1> bound;
    };
    std::vector<SFOverlayRectangleV1> rectangles;
    std::vector<Textured> textures;
    std::vector<Program> programs;
    std::uint64_t target_generation{};

    static std::int64_t Integer(const nlohmann::json& value,std::int64_t minimum,std::int64_t maximum) {
        if(!value.is_number_integer() || (value.is_number_unsigned() && value.get<std::uint64_t>()>std::uint64_t(INT64_MAX)))throw std::runtime_error("integer expected");
        const auto number=value.get<std::int64_t>();if(number<minimum || number>maximum)throw std::runtime_error("integer out of range");return number;
    }
    static std::uint64_t Identity(const nlohmann::json& value) {
        const auto& text=value.get_ref<const std::string&>();std::uint64_t result{};
        const auto [end,error]=std::from_chars(text.data(),text.data()+text.size(),result);
        if(error!=std::errc{} || end!=text.data()+text.size() || !result)throw std::runtime_error("nonzero decimal identity expected");return result;
    }
    template<class T> static std::vector<T> Bytes(const nlohmann::json& array,std::size_t maximum) {
        if(!array.is_array() || array.size()>maximum)throw std::runtime_error("byte/word span too large");
        std::vector<T> result;result.reserve(array.size());
        for(const auto& value:array)result.push_back(static_cast<T>(Integer(value,0,std::numeric_limits<T>::max())));return result;
    }
    static void Rectangle(const nlohmann::json& source,std::int32_t& x,std::int32_t& y,std::int32_t& width,std::int32_t& height) {
        x=static_cast<std::int32_t>(Integer(source.at("x"),-16384,16384));y=static_cast<std::int32_t>(Integer(source.at("y"),-16384,16384));
        width=static_cast<std::int32_t>(Integer(source.at("width"),1,16384));height=static_cast<std::int32_t>(Integer(source.at("height"),1,16384));
    }
    static Texture ReadTexture(const nlohmann::json& source) {
        Texture result;auto& value=result.value;
        value.resource_id=Identity(source.at("id"));value.revision=Identity(source.at("revision"));
        value.width=static_cast<std::uint32_t>(Integer(source.at("width"),1,4096));value.height=static_cast<std::uint32_t>(Integer(source.at("height"),1,4096));
        const auto texels=std::size_t(value.width)*value.height;
        result.pixels=Bytes<std::uint8_t>(source.at("rgba"),SF_OVERLAY_MAX_TEXTURE_BYTES);
        if(result.pixels.size()!=texels*4)throw std::runtime_error("texture RGBA extent mismatch");
        if(source.contains("coverage_mask")) {
            result.coverage=Bytes<std::uint8_t>(source.at("coverage_mask"),texels);
            if(result.coverage.size()!=texels || std::any_of(result.coverage.begin(),result.coverage.end(),[](auto b){return b>1;}))throw std::runtime_error("invalid coverage mask");
        }
        return result;
    }
    static std::shared_ptr<const OwnedOverlayScene> Read(const nlohmann::json& model) {
        if(!model.is_object())throw std::runtime_error("scene object expected");
        for(const auto& [key,_]:model.items())if(key!="rectangles" && key!="textures" && key!="programs" && key!="target_generation")throw std::runtime_error("unknown scene member");
        auto scene=std::make_shared<OwnedOverlayScene>();std::size_t commands{};
        const auto items=[&](const char* key){auto values=model.value(key,nlohmann::json::array());
            if(!values.is_array() || values.size()>4096 || (commands+=values.size())>4096)throw std::runtime_error("scene command limit exceeded");return values;};
        for(const auto& source:items("rectangles")) {
            SFOverlayRectangleV1 value{};Rectangle(source,value.x,value.y,value.width,value.height);
            value.rgba=static_cast<std::uint32_t>(Integer(source.at("rgba"),0,UINT32_MAX));scene->rectangles.push_back(value);
        }
        for(const auto& source:items("textures")) {
            Textured item;item.texture=ReadTexture(source.at("texture"));auto& value=item.command.base;
            Rectangle(source,value.x,value.y,value.width,value.height);
            const auto optional=[&](const char* key,std::int64_t fallback,std::int64_t min,std::int64_t max){return Integer(source.value(key,nlohmann::json(fallback)),min,max);};
            value.source_x=static_cast<std::uint32_t>(optional("source_x",0,0,item.texture.value.width));
            value.source_y=static_cast<std::uint32_t>(optional("source_y",0,0,item.texture.value.height));
            value.source_width=static_cast<std::uint32_t>(optional("source_width",0,0,item.texture.value.width-value.source_x));
            value.source_height=static_cast<std::uint32_t>(optional("source_height",0,0,item.texture.value.height-value.source_y));
            if((!value.source_width && value.source_x) || (!value.source_height && value.source_y))throw std::runtime_error("full texture source must start at zero");
            value.clip_center_x=static_cast<std::int32_t>(optional("clip_center_x",0,-16384,16384));value.clip_center_y=static_cast<std::int32_t>(optional("clip_center_y",0,-16384,16384));
            value.clip_radius=static_cast<std::int32_t>(optional("clip_radius",0,0,16384));
            value.tint_rgba=static_cast<std::uint32_t>(optional("tint_rgba",UINT32_MAX,0,UINT32_MAX));
            value.rotation_radians=source.value("rotation_radians",0.0f);if(!std::isfinite(value.rotation_radians))throw std::runtime_error("invalid rotation");
            item.command.square_clip=source.value("square_clip",false)?1:0;scene->textures.push_back(std::move(item));
        }
        for(const auto& source:items("programs")) {
            Program item;auto& value=item.command;Rectangle(source,value.x,value.y,value.width,value.height);
            if(value.x<0 || value.y<0)throw std::runtime_error("program destination must be nonnegative");
            value.shader={sizeof(SFOverlayShaderV1),1,Identity(source.at("id")),Identity(source.at("revision"))};
            item.words=Bytes<std::uint32_t>(source.at("spirv"),SF_OVERLAY_MAX_SHADER_BYTES/4);
            if(!ValidateOverlayFragment(item.words))throw std::runtime_error("unsupported overlay fragment shader");
            item.constants=Bytes<std::uint8_t>(source.value("constants",nlohmann::json::array()),SF_OVERLAY_MAX_PROGRAM_CONSTANT_BYTES);
            if(item.constants.size()%4)throw std::runtime_error("unaligned program constants");
            const auto textures=source.value("textures",nlohmann::json::array());
            if(!textures.is_array() || textures.size()>SF_OVERLAY_MAX_PROGRAM_TEXTURES)throw std::runtime_error("too many program textures");
            for(const auto& texture:textures)item.textures.push_back(ReadTexture(texture));scene->programs.push_back(std::move(item));
        }
        if(!scene->programs.empty())scene->target_generation=Identity(model.at("target_generation"));
        for(auto& item:scene->textures){item.texture.Bind();item.command.base.texture=item.texture.value;}
        for(auto& item:scene->programs){for(auto& texture:item.textures){texture.Bind();item.bound.push_back(texture.value);}
            item.command.shader.spirv=item.words.data();item.command.shader.spirv_bytes=static_cast<std::uint32_t>(item.words.size()*4);
            item.command.constants=item.constants.data();item.command.constant_bytes=static_cast<std::uint32_t>(item.constants.size());
            item.command.textures=item.bound.data();item.command.texture_count=static_cast<std::uint32_t>(item.bound.size());}
        return scene;
    }
    std::int32_t Draw(const SFOverlayEncoderV3& encoder,std::uint32_t width,std::uint32_t height) const {
        const auto& base=encoder.base.base;
        if(target_generation && target_generation!=base.device_generation)return 0;
        if(!rectangles.empty()){const auto status=base.draw_rectangles(base.context,base.owner_id,rectangles.data(),static_cast<std::uint32_t>(rectangles.size()));if(status)return status;}
        for(const auto& item:textures){const auto status=encoder.base.draw_textures_v2(base.context,base.owner_id,&item.command,1);if(status)return status;}
        for(const auto& item:programs){if(std::uint64_t(item.command.x)+item.command.width>width || std::uint64_t(item.command.y)+item.command.height>height)continue;
            const auto status=encoder.draw_programs(base.context,base.owner_id,&item.command,1);if(status)return status;}
        return 0;
    }
};
}
