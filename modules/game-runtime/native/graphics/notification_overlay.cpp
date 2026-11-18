#include "graphics/notification_overlay.h"
#include "graphics/render_overlay.h"
#include "abi/runtime_api.h"
#include <windows.h>
#include <array>
#include <algorithm>
#include <atomic>
#include <cstring>
#include <memory>
#include <mutex>

namespace NotificationOverlay {
namespace {
constexpr unsigned Width=480,Height=112,Capacity=4;
constexpr std::uint64_t Lifetime=5000;
struct Bitmap {std::array<std::uint8_t,Width*Height*4> rgba{};};
struct Notice {
    std::array<char,256> id{};unsigned id_size{};
    Bitmap bitmap;
    std::uint64_t token{},first_presented{};bool presented{};
};
struct State {
    std::mutex mutex;
    std::array<Notice,Capacity> notices{};
    unsigned head{},count{};std::uint64_t next{},drawn_frame{},drawn_token{};
    bool registered{};
    SFOverlayServiceV1 service{};
};
std::atomic<State*> published{};
std::atomic<bool> stopped{};
std::atomic<std::uint64_t> next_frame{},recording_frame{};
State& Storage() {
    // Initialized only by the producer, never from a graphics callback.
    static auto* state=new State;
    published.store(state,std::memory_order_release);return *state;
}
struct Canvas {
    HDC dc{};HBITMAP bitmap{};HFONT font{};HGDIOBJ old_bitmap{},old_font{};
    ~Canvas(){
        if(old_font&&old_font!=HGDI_ERROR)SelectObject(dc,old_font);
        if(old_bitmap&&old_bitmap!=HGDI_ERROR)SelectObject(dc,old_bitmap);
        if(font)DeleteObject(font);if(bitmap)DeleteObject(bitmap);if(dc)DeleteDC(dc);
    }
};
bool Text(const std::uint8_t* source,unsigned size,char* copy,wchar_t* wide,unsigned capacity) {
    if(!MapRuntime::ReadMemory(reinterpret_cast<std::uintptr_t>(source),copy,size)||std::memchr(copy,0,size))return false;
    const auto count=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,copy,int(size),wide,int(capacity-1));
    if(!count)return false;wide[count]=0;return true;
}
bool Raster(const wchar_t* title,const wchar_t* message,unsigned level,Bitmap& output) {
    Canvas canvas;canvas.dc=CreateCompatibleDC(nullptr);if(!canvas.dc)return false;
    BITMAPINFO info{};info.bmiHeader.biSize=sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth=Width;info.bmiHeader.biHeight=-LONG(Height);
    info.bmiHeader.biPlanes=1;info.bmiHeader.biBitCount=32;info.bmiHeader.biCompression=BI_RGB;
    void* pixels{};canvas.bitmap=CreateDIBSection(canvas.dc,&info,DIB_RGB_COLORS,&pixels,nullptr,0);
    if(!canvas.bitmap||!pixels)return false;
    canvas.old_bitmap=SelectObject(canvas.dc,canvas.bitmap);
    if(!canvas.old_bitmap||canvas.old_bitmap==HGDI_ERROR)return false;
    canvas.font=CreateFontW(-18,0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,CLIP_DEFAULT_PRECIS,ANTIALIASED_QUALITY,DEFAULT_PITCH,L"Segoe UI");
    if(!canvas.font)return false;
    canvas.old_font=SelectObject(canvas.dc,canvas.font);
    if(!canvas.old_font||canvas.old_font==HGDI_ERROR)return false;
    auto* bgra=static_cast<std::uint8_t*>(pixels);
    std::memset(bgra,24,Width*Height*4);
    if(!SetBkMode(canvas.dc,TRANSPARENT)||SetTextColor(canvas.dc,RGB(246,246,246))==CLR_INVALID)return false;
    RECT heading{16,10,LONG(Width-16),36},body{16,38,LONG(Width-16),LONG(Height-8)};
    if(!DrawTextW(canvas.dc,title,-1,&heading,DT_LEFT|DT_TOP|DT_SINGLELINE|DT_END_ELLIPSIS|DT_NOPREFIX)||
       !DrawTextW(canvas.dc,message,-1,&body,DT_LEFT|DT_TOP|DT_WORDBREAK|DT_END_ELLIPSIS|DT_NOPREFIX)||!GdiFlush())return false;
    constexpr std::uint32_t colors[]{0x8ac8ffff,0x8fdf9fff,0xffd57fff,0xff9292ff,0xc7b0ffff};
    for(unsigned y=0;y<Height;++y)for(unsigned x=0;x<Width;++x){
        const auto i=(std::size_t(y)*Width+x)*4;
        output.rgba[i]=bgra[i+2];output.rgba[i+1]=bgra[i+1];output.rgba[i+2]=bgra[i];
        output.rgba[i+3]=224;
        if(x<4)for(unsigned c=0;c<4;++c)output.rgba[i+c]=std::uint8_t(colors[level]>>(24-8*c));
    }
    return true;
}
std::int32_t Draw(void* context,const SFOverlayFrameV1* frame,const SFOverlayEncoderV1* encoder) {
    if(!context||!frame||!encoder||frame->size<sizeof(*frame)||frame->version!=1||!frame->width||!frame->height||
       encoder->size<sizeof(*encoder)||encoder->version!=1||!encoder->draw_textures||
       frame->device_generation!=encoder->device_generation)return 1;
    if(stopped.load(std::memory_order_acquire))return 0;
    auto& state=*static_cast<State*>(context);std::unique_lock lock(state.mutex,std::try_to_lock);
    if(!lock)return 0;
    const auto now=GetTickCount64();
    while(state.count){const auto& first=state.notices[state.head];
        if(!first.presented||now<first.first_presented||now-first.first_presented<Lifetime)break;
        state.head=(state.head+1)%Capacity;--state.count;
    }
    if(!state.count)return 0;
    auto& notice=state.notices[state.head];
    const auto width=(std::min)(Width,frame->width>32?frame->width-32:frame->width);
    const auto height=(std::min)(Height,frame->height>32?frame->height-32:frame->height);
    if(!width||!height)return 0;
    SFOverlayTextureCommandV1 command{};
    command.texture={notice.token,1,Width,Height,notice.bitmap.rgba.data(),notice.bitmap.rgba.size(),nullptr,0};
    command.x=int((frame->width-width)/2);command.y=frame->height>Height+32?16:0;
    command.width=int(width);command.height=int(height);command.tint_rgba=0xffffffff;
    const auto result=encoder->draw_textures(encoder->context,encoder->owner_id,&command,1);
    if(!result){state.drawn_frame=recording_frame.load(std::memory_order_acquire);state.drawn_token=notice.token;}
    return result;
}
bool Register(State& state) {
    if(state.registered){SFOverlayStatusV1 status{};return state.service.read_status(state.service.context,sizeof(status),&status)==0;}
    const auto owner=OverlayRuntime::Services().Open("shroudforge.native-notices",1);if(!owner)return false;
    SFOverlayServiceV1 table{};
    if(OverlayRuntime::Services().Query(owner,"shroudforge.overlay",1,sizeof(table),&table))return false;
    SFOverlayRegistrationV1 registration{};
    if(table.register_frame(table.context,Draw,&state,&registration))return false;
    state.service=table;state.registered=true;return true;
}
}
std::uint64_t BeginFrame() noexcept {
    const auto frame=next_frame.fetch_add(1,std::memory_order_relaxed)+1;
    recording_frame.store(frame,std::memory_order_release);return frame;
}
void Presented(std::uint64_t frame,std::uint64_t now) noexcept {
    auto* state=published.load(std::memory_order_acquire);if(!state||!frame)return;
    std::unique_lock lock(state->mutex,std::try_to_lock);if(!lock||!state->count||state->drawn_frame!=frame)return;
    auto& notice=state->notices[state->head];
    if(notice.token==state->drawn_token&&!notice.presented){notice.presented=true;notice.first_presented=now;}
}
void Stop() noexcept {
    stopped.store(true,std::memory_order_release);
    auto* state=published.load(std::memory_order_acquire);if(!state)return;
    std::lock_guard lock(state->mutex);state->count=0;
}
}
extern "C" KFC_RUNTIME_API std::int32_t __cdecl KfcRuntimePublishNotificationV1(
    const std::uint8_t* id,std::uint32_t id_size,const std::uint8_t* title,std::uint32_t title_size,
    const std::uint8_t* message,std::uint32_t message_size,std::uint32_t level) noexcept {
    using namespace NotificationOverlay;
    if(!id||!title||!message||!id_size||id_size>256||!title_size||title_size>120||!message_size||message_size>512||level>4)return 1;
    if(stopped.load(std::memory_order_acquire))return 3;
    try {
        char key[256]{},title_copy[120]{},message_copy[512]{};wchar_t heading[121]{},body[513]{};
        if(!MapRuntime::ReadMemory(reinterpret_cast<std::uintptr_t>(id),key,id_size))return 1;
        for(unsigned i=0;i<id_size;++i){const auto c=static_cast<unsigned char>(key[i]);
            if(!((c>='a'&&c<='z')||(c>='A'&&c<='Z')||(c>='0'&&c<='9')||c=='.'||c=='-'||c=='_'||c=='+'))return 1;
        }
        if(!Text(title,title_size,title_copy,heading,unsigned(std::size(heading)))||
           !Text(message,message_size,message_copy,body,unsigned(std::size(body))))return 1;
        auto bitmap=std::make_unique<Bitmap>();if(!Raster(heading,body,level,*bitmap))return 7;
        auto& state=Storage();std::lock_guard lock(state.mutex);
        if(stopped.load(std::memory_order_acquire))return 3;
        unsigned slot=Capacity;
        for(unsigned i=0;i<state.count;++i){const auto at=(state.head+i)%Capacity;const auto& n=state.notices[at];
            if(n.id_size==id_size&&!std::memcmp(n.id.data(),key,id_size)){slot=at;break;}}
        if(slot==Capacity&&state.count==Capacity)return 6;
        if(state.next==UINT64_MAX||!Register(state))return 3;
        if(slot==Capacity){slot=(state.head+state.count)%Capacity;++state.count;}
        auto& notice=state.notices[slot];std::memcpy(notice.id.data(),key,id_size);notice.id_size=id_size;
        notice.bitmap=*bitmap;notice.token=++state.next;notice.presented=false;notice.first_presented=0;
        return 0;
    }catch(...){return 7;}
}
