#pragma once
#include "startup_gate.h"
#include "greeneye_scan.h"
#include <string>
namespace GreeneyeEntry {
struct TicketV1 {unsigned bytes{},version{},kind{},reserved{};unsigned char binding[32]{};};
static_assert(sizeof(TicketV1)==48);
using Take=int(__cdecl*)(const wchar_t*,TicketV1*,unsigned);
using Load=std::uintptr_t(__cdecl*)(const wchar_t*,const TicketV1*);
using Confirm=bool(__cdecl*)(const wchar_t*,const TicketV1*,std::uintptr_t,bool);
struct Context {std::wstring root;TicketV1 ticket;Load load{};Confirm confirm{};};
inline bool Continue(Context& context,unsigned char* entry,const unsigned char* original) noexcept {
    const auto& ticket=context.ticket;
    if(ticket.bytes!=48 || ticket.version!=1 || ticket.kind!=2 || ticket.reserved || !context.load || !context.confirm)
        return StartupGate::Fail("original-loader-contract-unavailable");
    if(!GreeneyeDynamic::Begin(entry,original))return StartupGate::Fail(GreeneyeDynamic::Failure());
    const auto module=context.load(context.root.c_str(),&ticket);
    if(!module)return StartupGate::Fail("original-loader-load-failed");
    const bool witness=GreeneyeDynamic::Witness(module);
    const bool confirmed=context.confirm(context.root.c_str(),&ticket,module,witness);
    if(!witness || !confirmed)return StartupGate::Fail("original-loader-witness-refused");
    return GreeneyeDynamic::Finalize() || StartupGate::Fail(GreeneyeDynamic::Failure());
}
}
