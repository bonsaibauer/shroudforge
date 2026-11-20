#include "host/executable_span_validation.h"
#include <algorithm>
#include <array>
#include <chrono>
#include <cstring>
#include <limits>
#include <utility>

namespace ExecutableSpanValidation {
namespace {
constexpr std::int32_t Ok = 0;
constexpr std::int32_t InvalidArgument = 1;
constexpr std::int32_t AbiMismatch = 2;
constexpr std::int32_t Unavailable = 3;
constexpr std::int32_t BufferTooSmall = 5;
constexpr std::int32_t Busy = 6;
constexpr std::int32_t WrongThread = 8;
constexpr std::int32_t TooLarge = 10;
constexpr std::size_t MaxOwners = 4096;
constexpr std::size_t MaxGuardBytes = 512;

bool ValidPackage(std::string_view package) noexcept {
    if (package.empty() || package.size() > 256) return false;
    for (const unsigned char value : package) if (value < 32 || value == 127) return false;
    return true;
}
bool ZeroBytes(const std::uint8_t* bytes, std::size_t count) noexcept {
    std::uint8_t combined{};
    for (std::size_t i = 0; i < count; ++i) combined |= bytes[i];
    return combined == 0;
}
bool NonzeroBytes(const std::uint8_t* bytes, std::size_t count) noexcept {
    std::uint8_t combined{};
    for (std::size_t i = 0; i < count; ++i) combined |= bytes[i];
    return combined != 0;
}
bool SameSpan(const Registry::Span& left, const Registry::Span& right) noexcept;
}

struct Registry::Span {
    std::uint32_t rva{};
    std::uint32_t byte_count{};
    std::array<std::uint8_t, SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES> expected{};
    SFExecutableSpanValidationCallbackV1 callback{};
    void* callback_context{};
};

struct Registry::Owner {
    Registry* registry{};
    std::string package;
    std::uint64_t generation{};
    std::thread::id owner_thread{};
    std::size_t in_flight{};
    bool revoked{};
    std::vector<Span> spans;
    std::size_t ever_proofs{};
};

struct Registry::ProofLease {
    Owner* owner{};
    Span span;
    std::uint32_t prefix{};
    std::uint64_t session{}, id{}, adopted{};
    bool live{true};
};

namespace {
bool SameSpan(const Registry::Span& left, const Registry::Span& right) noexcept {
    return left.rva == right.rva && left.byte_count == right.byte_count &&
        left.callback == right.callback && left.callback_context == right.callback_context &&
        std::memcmp(left.expected.data(), right.expected.data(), left.byte_count) == 0;
}

bool ValidResponse(const SFExecutableSpanValidationResponseV1& response,
        const Registry::Span& span, const std::uint8_t* observed) noexcept {
    if (response.size != sizeof(response) ||
        response.version != SF_EXECUTABLE_SPAN_VALIDATION_VERSION_V1 || response.reserved != 0) return false;
    switch (response.result) {
    case SF_EXECUTABLE_SPAN_VALIDATION_UNCHANGED:
        return !std::memcmp(observed, span.expected.data(), span.byte_count) &&
            ZeroBytes(response.owner_identity, sizeof(response.owner_identity));
    case SF_EXECUTABLE_SPAN_VALIDATION_VERIFIED_OWNER:
        return NonzeroBytes(response.owner_identity, sizeof(response.owner_identity));
    default:
        return false;
    }
}
}

Registry::Registry(CodeReader reader, RegistrationGate gate,
        CallbackAddressValidator callback_validator) noexcept
    : Registry(reader,gate,callback_validator,nullptr,nullptr,nullptr,ProofLimits{}) {}
Registry::Registry(CodeReader reader, RegistrationGate gate,
        CallbackAddressValidator callback_validator,EngineHooks::OwnedSpanProofs* proofs,
        ProofCodePin pin,ProofDispatch dispatch,ProofLimits limits) noexcept
    : reader_(reader), registration_gate_(gate), callback_validator_(callback_validator),
      proofs_(proofs), proof_pin_(pin), proof_dispatch_(dispatch), proof_limits_(limits) {}
Registry::~Registry() = default;

Registry::Owner* Registry::FindLocked(std::string_view package, std::uint64_t generation) const noexcept {
    for (const auto& owner : owners_) {
        if (owner->generation == generation && owner->package == package) return owner.get();
    }
    return nullptr;
}
Registry::Owner* Registry::FindContextLocked(const void* context) const noexcept {
    for (const auto& owner : owners_) if (owner.get() == context) return owner.get();
    return nullptr;
}
Registry::Owner* Registry::CreateLocked(std::string_view package, std::uint64_t generation) noexcept {
    if (owners_.size() >= (std::min)(MaxOwners,proof_limits_.owners)) return nullptr;
    try {
        auto owner = std::make_unique<Owner>();
        owner->registry = this;
        owner->package.assign(package);
        owner->generation = generation;
        owner->owner_thread = std::this_thread::get_id();
        auto* pointer = owner.get();
        owners_.push_back(std::move(owner));
        return pointer;
    } catch (...) { return nullptr; }
}

std::int32_t Registry::Query(std::string_view package, std::uint64_t generation,
        std::uint32_t version, std::uint32_t capacity, void* output) noexcept {
    if (!ValidPackage(package) || !generation || !output) return InvalidArgument;
    if (version != SF_EXECUTABLE_SPAN_VALIDATION_VERSION_V1 &&
        version != SF_EXECUTABLE_SPAN_VALIDATION_VERSION_V2) return AbiMismatch;
    const bool v2=version==SF_EXECUTABLE_SPAN_VALIDATION_VERSION_V2;
    if (capacity < (v2?sizeof(SFExecutableSpanValidationServiceV2):sizeof(SFExecutableSpanValidationServiceV1))) return BufferTooSmall;
    if (v2 && (!proofs_ || !proof_pin_ || !proof_dispatch_)) return Unavailable;
    std::lock_guard lock(mutex_);
    auto* owner = FindLocked(package, generation);
    if (owner && owner->revoked) return Unavailable;
    if (!owner) owner = CreateLocked(package, generation);
    if (!owner) return Busy;
    if (owner->owner_thread != std::this_thread::get_id()) return WrongThread;
    if(v2) {
        const SFExecutableSpanValidationServiceV2 table{
            {sizeof(SFExecutableSpanValidationServiceV2),version,owner,&Registry::RegisterSpan},
            &Registry::AcquireProofLease};
        std::memcpy(output,&table,sizeof(table));
    } else {
        const SFExecutableSpanValidationServiceV1 table{
            sizeof(SFExecutableSpanValidationServiceV1),version,owner,&Registry::RegisterSpan};
        std::memcpy(output,&table,sizeof(table));
    }
    return Ok;
}

std::int32_t SF_EXECUTABLE_SPAN_VALIDATION_CALL Registry::AcquireProofLease(
        void* context,const SFExecutableSpanProofBindingV1* binding,
        std::uint32_t capacity,SFExecutableSpanProofLeaseV1* output) {
    if(!context)return InvalidArgument;
    auto* owner=static_cast<Owner*>(context);
    return owner->registry ? owner->registry->AcquireProof(owner,binding,capacity,output) : Unavailable;
}

std::int32_t Registry::AcquireProof(Owner* context,const SFExecutableSpanProofBindingV1* input,
        std::uint32_t capacity,SFExecutableSpanProofLeaseV1* output) noexcept {
    if(!input || !output)return InvalidArgument;
    if(capacity<sizeof(*output))return BufferTooSmall;
    const auto binding=*input;
    if(binding.size!=sizeof(binding) || binding.version!=1 || binding.reserved ||
        !binding.registered_byte_count || binding.registered_byte_count>32 ||
        !binding.owned_prefix_byte_count || binding.owned_prefix_byte_count>binding.registered_byte_count ||
        !ZeroBytes(binding.expected+binding.registered_byte_count,32-binding.registered_byte_count))return InvalidArgument;
    if(!proofs_ || !proof_dispatch_ || !proof_pin_ || !registration_gate_ || !registration_gate_())return Unavailable;
    Span span{};
    {
        std::lock_guard lock(mutex_);
        auto* owner=FindContextLocked(context);
        if(!owner || owner->revoked)return Unavailable;
        if(owner->owner_thread!=std::this_thread::get_id())return WrongThread;
        const auto found=std::find_if(owner->spans.begin(),owner->spans.end(),[&](const Span& s){
            return s.rva==binding.registered_rva && s.byte_count==binding.registered_byte_count &&
                !std::memcmp(s.expected.data(),binding.expected,s.byte_count);});
        if(found==owner->spans.end())return InvalidArgument;
        span=*found;
    }
    std::array<std::uint8_t,32> current{};
    if(!reader_ || !reader_(span.rva,current.data(),span.byte_count) ||
        std::memcmp(current.data(),span.expected.data(),span.byte_count) || !proof_pin_())return Unavailable;
    try {
        std::lock_guard gate(proofs_->Gate());
        std::lock_guard lock(mutex_);
        auto* owner=FindContextLocked(context);
        if(!owner || owner->revoked || !registration_gate_())return Unavailable;
        if(owner->owner_thread!=std::this_thread::get_id())return WrongThread;
        if(!std::any_of(owner->spans.begin(),owner->spans.end(),[&](const Span& s){return SameSpan(s,span);}))return Unavailable;
        if(owner->ever_proofs>=proof_limits_.per_owner || proof_leases_.size()>=proof_limits_.resident_leases ||
            next_proof_==UINT64_MAX)return TooLarge;
        for(const auto& lease:proof_leases_)
            if(lease->owner==owner && lease->span.rva==span.rva)return InvalidArgument;
        const auto session=proofs_->BeginStageLocked();
        if(!session)return TooLarge;
        auto lease=std::make_unique<ProofLease>();
        lease->owner=owner; lease->span=span; lease->prefix=binding.owned_prefix_byte_count;
        lease->session=session; lease->id=++next_proof_;
        SFExecutableSpanProofLeaseV1 result{sizeof(result),1,lease.get(),proof_dispatch_};
        proof_leases_.push_back(std::move(lease)); ++owner->ever_proofs;
        *output=result;return Ok;
    } catch (...) {return Busy;}
}

std::int32_t Registry::ValidateProof(void* context,const SFExecutableSpanProofQueryV1* input,
        std::uint32_t capacity,SFExecutableSpanProofResultV1* output) noexcept {
    if(!context || !input || !output)return InvalidArgument;
    if(capacity<sizeof(*output))return BufferTooSmall;
    const auto request=*input;
    if(request.size!=sizeof(request) || request.version!=1 || request.phase<1 || request.phase>3 ||
        !request.byte_count || request.byte_count>32 ||
        !ZeroBytes(request.observed_current+request.byte_count,32-request.byte_count))return InvalidArgument;
    if(!proofs_)return Unavailable;
    ProofLease* lease{};Owner* owner{};
    Span span{};std::uint32_t prefix{};std::uint64_t session{},adopted{};
    {
        std::lock_guard lock(mutex_);
        for(const auto& candidate:proof_leases_)if(candidate.get()==context){lease=candidate.get();break;}
        if(!lease)return InvalidArgument;
        owner=FindContextLocked(lease->owner);
        if(!owner || owner->revoked || !lease->live ||
            !std::any_of(owner->spans.begin(),owner->spans.end(),[&](const Span& s){return SameSpan(s,lease->span);}))return Unavailable;
        if(request.byte_count!=lease->span.byte_count)return InvalidArgument;
        span=lease->span;prefix=lease->prefix;session=lease->session;adopted=lease->adopted;
        ++owner->in_flight;
    }
    std::array<std::uint8_t,32> before{},after{};
    bool valid=reader_ && reader_(span.rva,before.data(),span.byte_count) &&
        !std::memcmp(before.data(),request.observed_current,span.byte_count) &&
        !std::memcmp(before.data()+prefix,span.expected.data()+prefix,span.byte_count-prefix);
    const bool unchanged=valid && !std::memcmp(before.data(),span.expected.data(),span.byte_count);
    std::shared_ptr<const EngineHooks::OwnedSpanProofs::Record> record;
    if(valid && (!unchanged || adopted)) {
        std::lock_guard gate(proofs_->Gate());
        record=proofs_->FindLocked(session,span.rva,{span.expected.data(),prefix});
    }
    if(valid && (!unchanged || adopted)) {
        valid=!unchanged && record && (!adopted || record->serial==adopted) &&
            !std::memcmp(before.data(),record->replacement.data(),prefix) &&
            record->verify && record->verify(record->witness.get());
    }
    if(valid)valid=reader_ && reader_(span.rva,after.data(),span.byte_count) &&
        !std::memcmp(before.data(),after.data(),span.byte_count);
    if(valid && record)valid=record->verify(record->witness.get());
    // Common success/closure ordering; no code reader/verifier under either lock.
    std::lock_guard gate(proofs_->Gate());
    std::lock_guard lock(mutex_);
    const bool live=lease->live && !owner->revoked && proofs_->SessionLiveLocked(session) &&
        std::any_of(owner->spans.begin(),owner->spans.end(),[&](const Span& s){return SameSpan(s,span);});
    valid=valid && live;
    if(record)valid=valid && proofs_->LiveLocked(record) &&
        (!lease->adopted || lease->adopted==record->serial);
    if(unchanged && lease->adopted)valid=false;
    SFExecutableSpanProofResultV1 result{sizeof(result),1,
        unchanged?SF_EXECUTABLE_SPAN_PROOF_UNCHANGED:SF_EXECUTABLE_SPAN_PROOF_VERIFIED_HOST_OWNED,0,{}};
    if(valid && record) {
        lease->adopted=record->serial;
        const std::uint64_t identity[]{session,record->serial,owner->generation,lease->id};
        static_assert(sizeof(identity)==sizeof(result.owner_identity));
        std::memcpy(result.owner_identity,identity,sizeof(identity));
    }
    if(!valid && (lease->adopted || !proofs_->SessionLiveLocked(session)))lease->live=false;
    if(valid)*output=result;
    --owner->in_flight;changed_.notify_all();
    return valid?Ok:Unavailable;
}

std::int32_t Registry::RegisterSpan(void* context,
        const SFExecutableSpanRegistrationV1* registration) noexcept {
    if (!context) return InvalidArgument;
    auto* owner = static_cast<Owner*>(context);
    if (!owner->registry) return InvalidArgument;
    return owner->registry->Register(owner, registration);
}

std::int32_t Registry::Register(Owner* context,
        const SFExecutableSpanRegistrationV1* registration) noexcept {
    if (!registration) return InvalidArgument;
    if (!registration_gate_ || !registration_gate_()) return Unavailable;
    {
        std::lock_guard lock(mutex_);
        auto* owner = FindContextLocked(context);
        if (!owner || owner->registry != this || owner->revoked) return Unavailable;
        if (owner->owner_thread != std::this_thread::get_id()) return WrongThread;
    }
    SFExecutableSpanRegistrationV1 copy{};
    std::memcpy(&copy, registration, sizeof(copy));
    if (copy.size != sizeof(copy) || copy.version != SF_EXECUTABLE_SPAN_VALIDATION_VERSION_V1 ||
        copy.byte_count == 0 || copy.byte_count > SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES ||
        !copy.callback || std::uint64_t(copy.rva) + copy.byte_count > (std::uint64_t(1) << 32) ||
        !ZeroBytes(copy.expected + copy.byte_count, sizeof(copy.expected) - copy.byte_count)) return InvalidArgument;
    if (!callback_validator_ || !callback_validator_(copy.callback)) return InvalidArgument;

    std::array<std::uint8_t, SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES> current{};
    if (!reader_ || !reader_(copy.rva, current.data(), copy.byte_count) ||
        std::memcmp(current.data(), copy.expected, copy.byte_count)) return Unavailable;

    std::lock_guard lock(mutex_);
    auto* owner = FindContextLocked(context);
    if (!owner || owner->registry != this || owner->revoked) return Unavailable;
    if (owner->owner_thread != std::this_thread::get_id()) return WrongThread;
    if (!registration_gate_()) return Unavailable;
    if (owner->spans.size() >= SF_EXECUTABLE_SPAN_VALIDATION_MAX_SPANS_PER_OWNER ||
        span_count_ >= SF_EXECUTABLE_SPAN_VALIDATION_MAX_SPANS_TOTAL) return TooLarge;
    const auto begin = std::uint64_t(copy.rva);
    const auto end = begin + copy.byte_count;
    for (const auto& candidate : owners_) {
        for (const auto& span : candidate->spans) {
            const auto other_begin = std::uint64_t(span.rva);
            const auto other_end = other_begin + span.byte_count;
            if (begin < other_end && other_begin < end) return InvalidArgument;
        }
    }
    Span span{};
    span.rva = copy.rva;
    span.byte_count = copy.byte_count;
    std::memcpy(span.expected.data(), copy.expected, copy.byte_count);
    span.callback = copy.callback;
    span.callback_context = copy.callback_context;
    try { owner->spans.push_back(span); }
    catch (...) { return Busy; }
    ++span_count_;
    return Ok;
}

std::int32_t Registry::Revoke(std::string_view package, std::uint64_t generation) noexcept {
    if (!ValidPackage(package) || !generation) return InvalidArgument;
    std::unique_lock<std::mutex> proof_gate;
    if(proofs_)proof_gate=std::unique_lock(proofs_->Gate());
    std::lock_guard lock(mutex_);
    auto* owner = FindLocked(package, generation);
    if (!owner) return Ok;
    if (owner->owner_thread != std::this_thread::get_id()) return WrongThread;
    owner->revoked = true;
    for(auto& lease:proof_leases_)if(lease->owner==owner)lease->live=false;
    changed_.notify_all();
    return Ok;
}

std::int32_t Registry::Drain(std::string_view package, std::uint64_t generation,
        std::uint32_t timeout_ms) noexcept {
    if (!ValidPackage(package) || !generation || timeout_ms > 250) return InvalidArgument;
    std::unique_lock lock(mutex_);
    auto* owner = FindLocked(package, generation);
    if (!owner) return Ok;
    if (owner->owner_thread != std::this_thread::get_id()) return WrongThread;
    if (!owner->revoked) return InvalidArgument;
    const auto drained = [&] { return owner->in_flight == 0; };
    if (!changed_.wait_for(lock, std::chrono::milliseconds(timeout_ms), drained)) return Busy;
    span_count_ -= owner->spans.size();
    owner->spans.clear();
    return Ok;
}

bool Registry::Invoke(Owner* owner, const Span& span, const std::uint8_t* observed,
        std::size_t guard_size, std::uint32_t phase,
        std::uint8_t identity[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES]) noexcept {
    std::array<std::uint8_t, MaxGuardBytes> before{};
    if (!guard_size || guard_size > before.size()) goto finish;
    if (observed) std::memcpy(before.data(), observed, guard_size);
    else if (!reader_ || !reader_(span.rva, before.data(), guard_size)) goto finish;

    {
        SFExecutableSpanValidationRequestV1 request{};
        request.size = sizeof(request);
        request.version = SF_EXECUTABLE_SPAN_VALIDATION_VERSION_V1;
        request.phase = phase;
        request.byte_count = span.byte_count;
        request.rva = span.rva;
        std::memcpy(request.expected, span.expected.data(), span.byte_count);
        std::memcpy(request.observed, before.data(), span.byte_count);

        SFExecutableSpanValidationResponseV1 response{};
        response.result = UINT32_MAX;
        response.reserved = UINT32_MAX;
        std::memset(response.owner_identity, 0xCC, sizeof(response.owner_identity));
        std::int32_t callback_status = -1;
        try { callback_status = span.callback(span.callback_context, &request, &response); }
        catch (...) { callback_status = -1; }

        std::array<std::uint8_t, MaxGuardBytes> after{};
        const bool stable = reader_ && reader_(span.rva, after.data(), guard_size) &&
            std::memcmp(before.data(), after.data(), guard_size) == 0;
        const bool response_valid = callback_status == 0 &&
            ValidResponse(response, span, before.data());

        std::lock_guard lock(mutex_);
        const bool live = owner && owner->registry == this && !owner->revoked &&
            owner->generation != 0 && owner->in_flight != 0 &&
            std::any_of(owner->spans.begin(), owner->spans.end(),
                [&](const Span& current) { return SameSpan(current, span); });
        if (owner && owner->in_flight) --owner->in_flight;
        changed_.notify_all();
        if (live && stable && response_valid) {
            std::memcpy(identity, response.owner_identity, sizeof(response.owner_identity));
            return true;
        }
        return false;
    }

finish:
    {
        std::lock_guard lock(mutex_);
        if (owner && owner->in_flight) --owner->in_flight;
        changed_.notify_all();
    }
    return false;
}

bool Registry::ValidateGuard(std::uint32_t rva, const std::uint8_t* expected,
        const std::uint8_t* observed, std::size_t guard_size, std::uint32_t phase,
        std::uint8_t owner_identity[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES]) noexcept {
    if (!expected || !observed || !guard_size || guard_size > MaxGuardBytes ||
        (phase != SF_EXECUTABLE_SPAN_PHASE_NORMALIZE &&
         phase != SF_EXECUTABLE_SPAN_PHASE_BEFORE_ORIGINAL &&
         phase != SF_EXECUTABLE_SPAN_PHASE_AFTER_ORIGINAL) ||
        std::uint64_t(rva) + guard_size > (std::uint64_t(1) << 32)) return false;

    Span span{};
    Owner* owner{};
    bool invoke{};
    {
        std::lock_guard lock(mutex_);
        for (const auto& candidate : owners_) {
            const auto found = std::find_if(candidate->spans.begin(), candidate->spans.end(),
                [&](const Span& value) { return value.rva == rva; });
            if (found == candidate->spans.end()) continue;
            owner = candidate.get();
            if (owner->revoked || owner->registry != this || guard_size < found->byte_count ||
                std::memcmp(expected, found->expected.data(), found->byte_count)) return false;
            span = *found;
            const bool prefix_unchanged = !std::memcmp(observed, span.expected.data(), span.byte_count);
            const bool suffix_unchanged = guard_size == span.byte_count ||
                !std::memcmp(expected + span.byte_count, observed + span.byte_count,
                    guard_size - span.byte_count);
            if (guard_size < span.byte_count || !suffix_unchanged) return false;
            invoke = phase != SF_EXECUTABLE_SPAN_PHASE_NORMALIZE || !prefix_unchanged;
            if (invoke) ++owner->in_flight;
            break;
        }
    }

    std::array<std::uint8_t, MaxGuardBytes> current{};
    if (!reader_ || !reader_(rva, current.data(), guard_size) ||
        std::memcmp(current.data(), observed, guard_size)) {
        if (invoke) {
            std::lock_guard lock(mutex_);
            if (owner->in_flight) --owner->in_flight;
            changed_.notify_all();
        }
        return false;
    }

    std::array<std::uint8_t, SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES> identity{};
    bool valid{};
    if (!owner) valid = std::memcmp(expected, current.data(), guard_size) == 0;
    else if (!invoke) {
        std::lock_guard lock(mutex_);
        const auto still_registered = !owner->revoked &&
            std::any_of(owner->spans.begin(), owner->spans.end(),
                [&](const Span& value) { return SameSpan(value, span); });
        valid = still_registered && std::memcmp(expected, current.data(), guard_size) == 0;
    } else {
        valid = Invoke(owner, span, current.data(), guard_size, phase, identity.data());
        if (valid) {
            std::array<std::uint8_t, MaxGuardBytes> normalized=current;
            std::memcpy(normalized.data(),span.expected.data(),span.byte_count);
            valid=std::memcmp(expected,normalized.data(),guard_size)==0;
        }
    }
    if (valid && owner_identity) std::memcpy(owner_identity, identity.data(), identity.size());
    return valid;
}

bool Registry::RevalidateAllRegistered() noexcept {
    std::vector<std::pair<Owner*, Span>> pending;
    try {
        std::lock_guard lock(mutex_);
        for (const auto& owner : owners_) {
            if (owner->spans.empty()) continue;
            if (owner->revoked || owner->registry != this) return false;
            for (const auto& span : owner->spans) pending.emplace_back(owner.get(), span);
        }
        for (const auto& [owner, _] : pending) ++owner->in_flight;
    } catch (...) { return false; }
    bool valid = true;
    for (const auto& [owner, span] : pending) {
        std::array<std::uint8_t, SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES> identity{};
        if (!Invoke(owner, span, nullptr, span.byte_count,
                SF_EXECUTABLE_SPAN_PHASE_NORMALIZE, identity.data())) valid = false;
    }
    return valid;
}
}
