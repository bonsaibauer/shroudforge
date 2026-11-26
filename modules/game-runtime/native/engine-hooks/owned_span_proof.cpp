#include "engine-hooks/owned_span_proof.h"
#include <cstring>
#include <limits>

namespace EngineHooks {
OwnedSpanProofs::OwnedSpanProofs() : OwnedSpanProofs(Limits{}) {}
OwnedSpanProofs::OwnedSpanProofs(Limits limits) : limits_(limits) {
    if (!limits_.sessions) { closed_=true; session_=0; ever_sessions_=0; }
}
std::uint64_t OwnedSpanProofs::BeginStageLocked() {
    if (!closed_) return session_;
    if (ever_sessions_ >= limits_.sessions || next_session_ == UINT64_MAX) return 0;
    session_=++next_session_; ++ever_sessions_; closed_=false;
    return session_;
}
std::uint64_t OwnedSpanProofs::SessionLocked() const noexcept { return closed_ ? 0 : session_; }
bool OwnedSpanProofs::SessionLiveLocked(std::uint64_t session) const noexcept {
    return session && !closed_ && session==session_;
}
bool OwnedSpanProofs::LiveLocked(const std::shared_ptr<const Record>& record) const noexcept {
    return record && record->live && SessionLiveLocked(record->session);
}
std::shared_ptr<const OwnedSpanProofs::Record> OwnedSpanProofs::FindRangeLocked(
        std::uint64_t session,std::uint32_t rva,std::size_t count,bool& conflict) const noexcept {
    conflict=false;
    if (!SessionLiveLocked(session) || !count || count>32 || rva>UINT32_MAX-count) return {};
    std::shared_ptr<const Record> exact;
    for (const auto& record:records_) {
        if (!record->live || record->session!=session) continue;
        const auto end=std::uint64_t(rva)+count;
        const auto other_end=std::uint64_t(record->rva)+record->count;
        if (rva>=other_end || record->rva>=end) continue;
        if (exact || record->rva!=rva || record->count!=count) {conflict=true;return {};}
        exact=record;
    }
    return exact;
}
std::shared_ptr<const OwnedSpanProofs::Record> OwnedSpanProofs::FindLocked(
        std::uint64_t session,std::uint32_t rva,std::span<const std::uint8_t> original) const noexcept {
    bool conflict{};
    const auto exact=FindRangeLocked(session,rva,original.size(),conflict);
    return exact && original.data() &&
        !std::memcmp(exact->original.data(),original.data(),original.size()) ? exact : nullptr;
}
bool OwnedSpanProofs::Publish(std::uint32_t rva,std::span<const std::uint8_t> original,
        std::span<const std::uint8_t> replacement,std::shared_ptr<const void> witness,Verifier verify) noexcept {
    if (!witness || !verify || original.empty() || original.size()>32 ||
        original.size()!=replacement.size() || rva>UINT32_MAX-original.size() ||
        !std::memcmp(original.data(),replacement.data(),original.size())) return false;
    try {
        std::lock_guard lock(gate_);
        if (closed_ || ever_records_>=limits_.records || next_serial_==UINT64_MAX) return false;
        const auto end=std::uint64_t(rva)+original.size();
        for (const auto& record:records_) {
            if (record->session!=session_) continue;
            if (rva<std::uint64_t(record->rva)+record->count && record->rva<end) return false;
        }
        auto record=std::make_shared<Record>();
        record->session=session_; record->serial=++next_serial_; record->rva=rva;
        record->count=static_cast<std::uint32_t>(original.size());
        std::memcpy(record->original.data(),original.data(),original.size());
        std::memcpy(record->replacement.data(),replacement.data(),replacement.size());
        record->witness=std::move(witness); record->verify=verify;
        records_.push_back(std::move(record)); ++ever_records_; return true;
    } catch (...) { return false; }
}
void OwnedSpanProofs::Close() noexcept {
    std::lock_guard lock(gate_); closed_=true;
}
void OwnedSpanProofs::Retire(std::uint64_t serial) noexcept {
    std::lock_guard lock(gate_);
    for (const auto& record:records_) if (record->serial==serial) record->live=false;
}
OwnedSpanProofs& ExecutableProofs() noexcept {
    static auto* proofs=new OwnedSpanProofs;
    return *proofs;
}
}
