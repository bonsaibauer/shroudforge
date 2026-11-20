#include "host/executable_span_validation.h"
#include "engine-hooks/shared_call_proof.h"
#include <array>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <thread>

namespace {
int failures{},checks{};
#define CHECK(x) do { ++checks; if(!(x)){++failures;std::fprintf(stderr,"proof assertion line %d: %s\n",__LINE__,#x);} } while(false)
struct Fixture;
Fixture* active{};
bool Read(std::uint32_t,std::uint8_t*,std::size_t) noexcept;
bool Gate() noexcept;
bool Executable(SFExecutableSpanValidationCallbackV1 value) noexcept {return value!=nullptr;}
bool Pin() noexcept {return true;}
int32_t Dispatch(void*,const SFExecutableSpanProofQueryV1*,uint32_t,SFExecutableSpanProofResultV1*);
int32_t Callback(void*,const SFExecutableSpanValidationRequestV1*,SFExecutableSpanValidationResponseV1*);
struct Witness {
    std::atomic<bool> valid{true};
    std::mutex mutex;
    std::condition_variable changed;
    bool block{},entered{},release{};
};
bool Verify(const void* context) noexcept {
    auto& value=*const_cast<Witness*>(static_cast<const Witness*>(context));
    if(value.block){std::unique_lock lock(value.mutex);value.entered=true;value.changed.notify_all();
        value.changed.wait(lock,[&]{return value.release;});}
    return value.valid.load();
}
struct Fixture {
    static constexpr std::uint32_t Base=0x1000;
    std::array<std::uint8_t,512> code{};
    std::array<std::uint8_t,10> original{};
    std::array<std::uint8_t,5> replacement{0xe8,0x81,0x42,0,0};
    bool stage{true},readable{true};
    unsigned reads{},mutate_read{};
    EngineHooks::OwnedSpanProofs proofs;
    ExecutableSpanValidation::Registry registry;
    SFExecutableSpanValidationServiceV2 service{};
    SFExecutableSpanProofLeaseV1 lease{};
    std::shared_ptr<Witness> witness=std::make_shared<Witness>();
    std::array<unsigned,12> contexts{};
    std::array<unsigned,64> order{};unsigned calls{};
    unsigned descriptor{9};
    explicit Fixture(ExecutableSpanValidation::Registry::ProofLimits limits={},
            EngineHooks::OwnedSpanProofs::Limits store_limits={})
        : proofs(store_limits),registry(Read,Gate,Executable,&proofs,Pin,Dispatch,limits) {
        active=this;code.fill(0x90);for(unsigned i=0;i<10;++i)original[i]=std::uint8_t(0x20+i);
        original[0]=original[5]=0xe8;
        for(unsigned i=0;i<12;++i)std::memcpy(code.data()+16*i,original.data(),10);
        CHECK(registry.Query("fixture",7,2,sizeof(service),&service)==0);
    }
    std::uint32_t Rva()const{return Base+16*descriptor;}
    void Register(unsigned count=12) {
        for(unsigned i=0;i<count;++i){contexts[i]=i;
            SFExecutableSpanRegistrationV1 r{};r.size=sizeof(r);r.version=1;r.rva=Base+16*i;r.byte_count=10;
            std::memcpy(r.expected,original.data(),10);r.callback=Callback;r.callback_context=&contexts[i];
            CHECK(service.base.register_span(service.base.context,&r)==0);
        }
    }
    int Acquire(SFExecutableSpanProofLeaseV1& out,unsigned index=9) {
        SFExecutableSpanProofBindingV1 b{};b.size=sizeof(b);b.version=1;b.registered_rva=Base+16*index;
        b.registered_byte_count=10;b.owned_prefix_byte_count=5;std::memcpy(b.expected,original.data(),10);
        return service.acquire_owned_prefix_lease(service.base.context,&b,sizeof(out),&out);
    }
    void Acquire(){CHECK(Acquire(lease)==0);}
    void Publish(){CHECK(proofs.Publish(Rva(),{original.data(),5},replacement,witness,Verify));
        std::memcpy(code.data()+16*descriptor,replacement.data(),5);}
    SFExecutableSpanProofQueryV1 Request()const {
        SFExecutableSpanProofQueryV1 q{};q.size=sizeof(q);q.version=1;q.phase=1;q.byte_count=10;
        std::memcpy(q.observed_current,code.data()+16*descriptor,10);return q;
    }
    int Query(SFExecutableSpanProofResultV1& out) {
        auto q=Request();return lease.validate(lease.context,&q,sizeof(out),&out);
    }
};
bool Read(std::uint32_t rva,std::uint8_t* out,std::size_t size) noexcept {
    auto& f=*active;
    if(!f.readable || rva<Fixture::Base || size>f.code.size() || rva-Fixture::Base>f.code.size()-size)return false;
    if(++f.reads==f.mutate_read)f.code[rva-Fixture::Base]^=0x40;
    std::memcpy(out,f.code.data()+rva-Fixture::Base,size);return true;
}
bool Gate()noexcept{return active->stage;}
int32_t Dispatch(void* c,const SFExecutableSpanProofQueryV1* q,uint32_t n,SFExecutableSpanProofResultV1* r){
    return active->registry.ValidateProof(c,q,n,r);
}
int32_t Callback(void* c,const SFExecutableSpanValidationRequestV1* q,SFExecutableSpanValidationResponseV1* r){
    auto& f=*active;const auto index=*static_cast<unsigned*>(c);f.order[f.calls++]=index;
    SFExecutableSpanValidationResponseV1 result{sizeof(result),1,SF_EXECUTABLE_SPAN_VALIDATION_UNCHANGED,0,{}};
    if(index==f.descriptor){
        SFExecutableSpanProofQueryV1 request{sizeof(request),1,q->phase,q->byte_count,{}};
        std::memcpy(request.observed_current,q->observed,q->byte_count);
        SFExecutableSpanProofResultV1 proof{};
        if(f.lease.validate(f.lease.context,&request,sizeof(proof),&proof))return 1;
        if(proof.classification==SF_EXECUTABLE_SPAN_PROOF_VERIFIED_HOST_OWNED){
            result.result=SF_EXECUTABLE_SPAN_VALIDATION_VERIFIED_OWNER;
            std::memcpy(result.owner_identity,proof.owner_identity,32);
        }
    }
    *r=result;return 0;
}
bool Zero(const std::uint8_t* a){std::uint8_t b{};for(unsigned i=0;i<32;++i)b|=a[i];return b==0;}

void PendingAndAllDescriptors(){
    Fixture f;f.Register();f.Acquire();
    SFExecutableSpanProofResultV1 a{};CHECK(f.Query(a)==0);CHECK(a.classification==1 && Zero(a.owner_identity));
    // Ordinary initialization does not open/close a native session.
    {std::lock_guard lock(f.proofs.Gate());CHECK(f.proofs.SessionLocked()==1);}
    f.Publish();f.stage=false;CHECK(f.Query(a)==0);CHECK(a.classification==2 && !Zero(a.owner_identity));
    SFExecutableSpanProofResultV1 b{};std::thread worker([&]{CHECK(f.Query(b)==0);});worker.join();
    CHECK(std::memcmp(a.owner_identity,b.owner_identity,32)==0);
    CHECK(f.registry.RevalidateAllRegistered());CHECK(f.calls==12);
    for(unsigned i=0;i<12;++i)CHECK(f.order[i]==i);
    for(unsigned phase=1;phase<=3;++phase){std::uint8_t identity[32]{};
        CHECK(f.registry.ValidateGuard(f.Rva(),f.original.data(),f.code.data()+16*f.descriptor,10,phase,identity));
        CHECK(std::memcmp(identity,a.owner_identity,32)==0);}
}
void RefusalsAndRetirement(){
    Fixture f;f.Register();f.Acquire();SFExecutableSpanProofResultV1 out;std::memset(&out,0xa5,sizeof(out));auto before=out;
    auto q=f.Request();q.observed_current[31]=1;CHECK(f.lease.validate(f.lease.context,&q,sizeof(out),&out)==1);
    CHECK(std::memcmp(&out,&before,sizeof(out))==0);
    q=f.Request();CHECK(f.lease.validate(reinterpret_cast<void*>(0x1234),&q,sizeof(out),&out)==1);
    f.stage=false;SFExecutableSpanProofLeaseV1 second{};CHECK(f.Acquire(second)!=0);f.stage=true;
    int other{};std::thread wrong([&]{other=f.Acquire(second);});wrong.join();CHECK(other==8);
    f.Publish();auto altered=f.Request();altered.observed_current[0]^=1;
    CHECK(f.lease.validate(f.lease.context,&altered,sizeof(out),&out)!=0);CHECK(std::memcmp(&out,&before,sizeof(out))==0);
    f.code[16*f.descriptor+7]^=1;CHECK(f.Query(out)!=0);f.code[16*f.descriptor+7]^=1;
    f.witness->valid=false;CHECK(f.Query(out)!=0);f.witness->valid=true;
    f.readable=false;CHECK(f.Query(out)!=0);f.readable=true;
    f.mutate_read=f.reads+2;CHECK(f.Query(out)!=0);f.code[16*f.descriptor]^=0x40;f.mutate_read=0;
    CHECK(f.Query(out)==0);
    std::memcpy(f.code.data()+16*f.descriptor,f.original.data(),10);
    CHECK(f.Query(out)!=0);std::memcpy(f.code.data()+16*f.descriptor,f.replacement.data(),5);CHECK(f.Query(out)!=0);
}
void CloseAndDrain(bool package){
    Fixture f;f.Register();f.Acquire();f.Publish();f.witness->block=true;
    SFExecutableSpanProofResultV1 out;std::memset(&out,0xa5,sizeof(out));const auto before=out;int status{};
    std::thread query([&]{status=f.Query(out);});
    {std::unique_lock lock(f.witness->mutex);CHECK(f.witness->changed.wait_for(lock,std::chrono::seconds(3),[&]{return f.witness->entered;}));}
    if(package)CHECK(f.registry.Revoke("fixture",7)==0);
    else {
        f.proofs.Close();
        CHECK(f.registry.Drain("fixture",7,0)==1); // Package drain requires its own revoke.
        CHECK(f.registry.Revoke("fixture",7)==0);
    }
    CHECK(f.registry.Drain("fixture",7,0)==6);
    {std::lock_guard lock(f.witness->mutex);f.witness->release=true;f.witness->changed.notify_all();}
    query.join();CHECK(status!=0);CHECK(std::memcmp(&out,&before,sizeof(out))==0);
    CHECK(f.registry.Drain("fixture",7,0)==0);CHECK(f.Query(out)!=0);
}
void BudgetsAndIdentity(){
    Fixture f({2,1,1},{1,1});f.Register();f.Acquire();
    SFExecutableSpanProofLeaseV1 second{};CHECK(f.Acquire(second,0)==10);
    f.Publish();SFExecutableSpanProofResultV1 out{};CHECK(f.Query(out)==0);const auto committed=out;
    std::shared_ptr<const EngineHooks::OwnedSpanProofs::Record> record;
    {std::lock_guard lock(f.proofs.Gate());record=f.proofs.FindLocked(1,f.Rva(),{f.original.data(),5});}
    CHECK(record!=nullptr);f.proofs.Retire(record->serial);CHECK(f.Query(out)!=0);
    CHECK(std::memcmp(&committed,&out,sizeof(out))==0);
    CHECK(!f.proofs.Publish(f.Rva()+16,{f.original.data(),5},f.replacement,f.witness,Verify));
    CHECK(f.registry.Revoke("fixture",7)==0);CHECK(f.registry.Drain("fixture",7,0)==0);
    CHECK(f.Acquire(second)!=0);CHECK(f.Query(out)!=0);f.proofs.Close();
    {std::lock_guard lock(f.proofs.Gate());CHECK(f.proofs.BeginStageLocked()==0);}
}

EngineHooks::SharedCallWitness sample;
std::array<std::uint8_t,5> call_bytes;
std::array<std::uint8_t,14> relay_bytes;
bool region_ok=true,resident_ok=true;
bool ReadWitness(std::uintptr_t at,void* out,std::size_t n) noexcept {
    if(at==sample.target && n==5){std::memcpy(out,call_bytes.data(),n);return true;}
    if(at==sample.allocation && n==14){std::memcpy(out,relay_bytes.data(),n);return true;}return false;
}
bool Region(std::uintptr_t,std::size_t,std::uintptr_t,bool)noexcept{return region_ok;}
bool Resident(std::uintptr_t,std::uintptr_t)noexcept{return resident_ok;}
bool CheckWitness(){return EngineHooks::VerifySharedCallWitness(sample,ReadWitness,Region,Resident);}
void NativeWitnessPredicates(){
    sample={};sample.image=0x2000;sample.target=0x2100;sample.original_target=0x2400;
    sample.allocation=0x3100;sample.handler=0x4200;sample.resident=0x4000;
    sample.original[0]=sample.replacement[0]=0xe8;
    std::int32_t a=0x2400-0x2105,b=0x3100-0x2105;
    std::memcpy(sample.original.data()+1,&a,4);std::memcpy(sample.replacement.data()+1,&b,4);
    sample.relay[0]=0xff;sample.relay[1]=0x25;std::memcpy(sample.relay.data()+6,&sample.handler,8);
    call_bytes=sample.replacement;relay_bytes=sample.relay;CHECK(CheckWitness());
    for(auto& byte:call_bytes){byte^=1;CHECK(!CheckWitness());byte^=1;}
    for(auto& byte:relay_bytes){byte^=1;CHECK(!CheckWitness());byte^=1;}
    ++sample.original_target;CHECK(!CheckWitness());--sample.original_target;
    ++sample.handler;CHECK(!CheckWitness());--sample.handler;
    region_ok=false;CHECK(!CheckWitness());region_ok=true;resident_ok=false;CHECK(!CheckWitness());resident_ok=true;
    CHECK(CheckWitness());
}
}
int RunOwnedSpanProofTests() {
    static_assert(sizeof(SFExecutableSpanValidationServiceV2)==32);
    static_assert(sizeof(SFExecutableSpanProofBindingV1)==56);
    static_assert(sizeof(SFExecutableSpanProofLeaseV1)==24);
    static_assert(sizeof(SFExecutableSpanProofQueryV1)==48);
    static_assert(sizeof(SFExecutableSpanProofResultV1)==48);
    PendingAndAllDescriptors();RefusalsAndRetirement();CloseAndDrain(true);CloseAndDrain(false);
    BudgetsAndIdentity();NativeWitnessPredicates();
    std::printf("Owned span proof: %d checks, %d failures\n",checks,failures);return failures?1:0;
}
