#pragma once
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <string_view>
// Exact selected current HUD dependencies, adapted from accepted557 profile2.
// Renderer entries stay original; only a verified framework-owned CALL can be normalized.
namespace MapRuntime::CurrentHud5d9ef2f1 {
using Reader=bool(*)(const std::uint8_t*,void*,std::size_t);
using Normalizer=bool(*)(std::uint32_t,const std::uint8_t*,const std::uint8_t*,std::size_t);
struct Pin{std::size_t rva;std::string_view bytes;};
using namespace std::literals;
inline constexpr Pin Pins[]={
{0xc603d0,"\x4c\x8b\xdc\x55\x49\x8d\xab\xe8\xfd\xff\xff\x48\x81\xec\x10\x03\x00\x00"sv},
{0xc603e2,"\x48\x8b\x05\xbf\xf0\xbd\x01\x48\x8b\x88\xd0\x00\x00\x00\x48\x8b\x01\x48\x85\xc0\x0f\x84\x94\x0e\x00\x00\x4d\x89\x73\xc8\x4c\x8b\x70\x38\x4c\x89\xb5\x30\x02\x00\x00\x4d\x85\xf6\x0f\x84\x74\x0e\x00\x00\x4d\x89\x63\xd8"sv},
{0xc60418,"\x4d\x8d\xa6\xd8\x1c\x00\x00\x41\x0f\xb6\x04\x24\x4c\x89\x65\x80\x3c\x33\x0f\x84\x50\x0e\x00\x00\x84\xc0\x0f\x84\x48\x0e\x00\x00"sv},
{0xbd1116,"\x48\x89\x05\x8b\xe3\xc6\x01"sv},
{0xbd1639,"\x4c\x89\x25\x68\xde\xc6\x01"sv},
{0xbd145b,"\x41\x0f\xb6\x47\x0c\xff\xc8\x83\xf8\x06\x0f\x87\xa4\x01\x00\x00\x48\x8d\x15\x8e\xeb\x42\xff\x48\x98\x8b\x8c\x82\xb4\x16\xbd\x00\x48\x03\xca\xff\xe1"sv},
{0xbd16b4,"\x80\x14\xbd\x00\x8a\x14\xbd\x00\x94\x14\xbd\x00\x9e\x14\xbd\x00\x0b\x15\xbd\x00\x78\x15\xbd\x00\x0a\x16\xbd\x00"sv},
{0xbd160a,"\xe8\xc1\xed\x08\x00\xe8\x2c\xb2\x0b\x00"sv},
{0xd80f30,"\x48\x89\x0d\x71\xe5\xab\x01\xc3"sv}
};
inline bool Match(const std::uint8_t* image,std::size_t size,const std::uint8_t* callPrefix,
        std::size_t callSize,Reader read,Normalizer normalize=nullptr) {
    if(!image || !read)return false;

    if(callPrefix || callSize){
        if(!callPrefix || callSize!=5 || std::memcmp(callPrefix,Pins[7].bytes.data(),5))return false;
    }
    for(const auto& pin:Pins){
        if(pin.rva>size || pin.bytes.size()>size-pin.rva || pin.bytes.size()>64)return false;
        std::array<std::uint8_t,64> actual{};
        if(!read(image+pin.rva,actual.data(),pin.bytes.size()))return false;
        if(callPrefix && pin.rva==0xbd160a)std::memcpy(actual.data(),callPrefix,5);
        if(std::memcmp(actual.data(),pin.bytes.data(),pin.bytes.size()) &&
            (!normalize || pin.rva>UINT32_MAX || !normalize(static_cast<std::uint32_t>(pin.rva),
                reinterpret_cast<const std::uint8_t*>(pin.bytes.data()),actual.data(),pin.bytes.size())))return false;
    }return true;
}
}
