#pragma once
#include <cstdint>
#include <cstddef>
#include <string_view>
namespace MapRuntime::Current5d9ef2f1 {
inline constexpr char Sha[]="5d9ef2f140b8ed49e30dbee655841b89d08baec20f799a1817e171a18a850c77";
inline constexpr std::size_t ImageSize=0x2a3f000;
inline constexpr std::uintptr_t Camera=0x2b7900,CameraResume=0x2b7912,EntityArray=0x830;
inline constexpr std::uintptr_t IterInit=0x9c7b80,IterNext=0x9c2780,GetWorld=0x9bfb80,Lookup=0x9b2b50;
inline constexpr std::uintptr_t Remote=0x31d390,Waypoints=0x314000,Clock=0x2b80e0,World=0x304470;
inline constexpr std::uintptr_t WeatherCall=0x3031a8,WeatherSample=0xaaa4d0,FogCall=0x30c734,FogUpdate=0x3a2780,Discovery=0x41c3b2;
inline constexpr std::size_t CustomList=0x396a28,WorldList=0x36ea00,SecondaryWorldList=0x3bea50,DeathList=0x3e6a78;
constexpr bool Selected(std::string_view sha) noexcept{return sha==Sha;}
// Static capability data only. No held preparation or product/runtime acceptance.
}
