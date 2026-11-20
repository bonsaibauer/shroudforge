#pragma once
#include <cstdint>
#include <cstddef>
#include <string_view>
namespace MapRuntime::Current5d9ef2f1 {
inline constexpr char Sha[]="5d9ef2f140b8ed49e30dbee655841b89d08baec20f799a1817e171a18a850c77";
inline constexpr std::size_t ImageSize=0x2a3f000;
inline constexpr std::uint32_t Timestamp=1793791052;
inline constexpr std::uintptr_t Camera=0x2b7900,CameraResume=0x2b7912,EntityArray=0x830;
inline constexpr std::uintptr_t IterInit=0x9c7b80,IterNext=0x9c2780,GetWorld=0x9bfb80,Lookup=0x9b2b50;
inline constexpr std::uintptr_t Remote=0x31d390,Waypoints=0x314000,Clock=0x2b80e0,World=0x304470;
inline constexpr std::uintptr_t WeatherCall=0x3031a8,WeatherSample=0xaaa4d0,FogCall=0x30c734,FogUpdate=0x3a2780,Discovery=0x41c3b2;
inline constexpr std::size_t CustomList=0x396a28,WorldList=0x36ea00,SecondaryWorldList=0x3bea50,DeathList=0x3e6a78;
inline constexpr std::uintptr_t HudCall=0xbd160a,Renderer=0xc603d0,UiGlobal=0x283f4a8;
struct NamedSystem{std::size_t descriptor;const char* name;std::size_t callback;};
inline constexpr NamedSystem Systems[]={
{0x1E24E40,"player_camera_transform",0x2B7900},
{0x1E2EDC0,"player_waypoints_ui",0x314000},
{0x1E25010,"player_daytime_ui",0x2B80E0},
{0x1E28FE0,"client_update_daytime",0x307C40},
{0x1E25F20,"ambient_effects",0x302E30},
{0x1E2B320,"fog_of_war",0x30C670},
{0x1E26D90,"clear_map_markers_for_ui",0x304470},
{0x1E29DD0,"custom_map_markers_ui",0x3087F0},
{0x1E30330,"replicate_map_markers_for_ui",0x3161B0},
{0x1E347C0,"update_ui_data",0x31D390},
{0x1E0B0A0,"knowledge_query_mapmarker_visibility",0x41C150}
};
constexpr bool Selected(std::string_view sha) noexcept{return sha==Sha;}
// Static capability data only. No held preparation or product/runtime acceptance.
}
