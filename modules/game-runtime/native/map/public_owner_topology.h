// Included inside PublicCreativeCompat after its exact descriptors/identity API.
// One shared finite state per whole observation; no foreign graph is cached.
namespace FiniteGraph
{
    struct Limits { unsigned reads=64; std::size_t bytes=4096; };
    struct Stats { unsigned reads=0,nearCount=0,greenCount=0,creativeCount=0;std::size_t bytes=0; };
    // Optional control-plane diagnostic only. Callback callers pass nullptr.
    struct Diagnostic {
        const char* reason="not-inspected";
        unsigned beforeModules{},afterModules{},retainedModules{},reads{};
    };
    struct Module
    {
        HMODULE reference=nullptr;std::uintptr_t base=0,callback=0;
        const Profile* profile=nullptr;FileCache* cache=nullptr;unsigned family=0;
        bool deferred=false;Digest identity{};TrustedImageBytes expected{},mapped{};
    };
    struct Observation
    {
        std::array<unsigned char,4096> bytes{};std::size_t used=0;
        Limits limits{};Stats stats{};IdentityStamp stamp{};
        std::uintptr_t renderer=0;std::array<unsigned char,18> live{};
        unsigned rootMode=0,rootKind=3,families=0,moduleCount=0;
        bool allGreenNew=true,requireNewGreen=false,complete=false;
        std::array<Module,3> modules{};
        std::array<std::uintptr_t,8> visited{};unsigned visits=0;
        explicit Observation(Limits value):limits(value),stamp(CurrentIdentityStamp()){}
        ~Observation(){for(auto& m:modules)if(m.reference)FreeLibrary(m.reference);}
        bool Read(std::uintptr_t at,void* to,std::size_t n,bool code=true)
        {
            if(stats.reads>=64 || stats.reads>=limits.reads)return OriginalOwnerFailure::Refuse("graph-read","read-budget");
            ++stats.reads;
            OriginalOwnerFailure::Read(visits,stats.reads);
#ifdef MM_PUBLIC_OWNER_TOPOLOGY_READ
            MM_PUBLIC_OWNER_TOPOLOGY_READ(at,n);
#endif
            return OriginalOwnerFailure::Result(Copy(at,to,n,code),"graph-read","memory-copy");
        }
        bool Add(const void* p,std::size_t n)
        {
            if(!p || used>bytes.size() || n>bytes.size()-used || used>limits.bytes || n>limits.bytes-used)return OriginalOwnerFailure::Refuse("graph-proof","byte-budget");
            std::memcpy(bytes.data()+used,p,n);used+=n;stats.bytes=used;return true;
        }
        template<class T> bool Add(const T& x){return Add(&x,sizeof(x));}
        bool Visit(std::uintptr_t at)
        {
            if(!Range(at,1) || visits==visited.size())return OriginalOwnerFailure::Refuse("graph-visit","range-or-node-limit");
            for(unsigned i=0;i<visits;++i)if(visited[i]==at)return OriginalOwnerFailure::Refuse("graph-visit","cycle");
            visited[visits++]=at;return true;
        }
        static bool New(const Profile* p){return p==&Find125 || p==&Teleport117 || p==&Find128 || p==&Teleport1110 || p==&Find1210 || p==&Teleport1111;}
        static bool Mov(const unsigned char* p)
        {
            if(p[0]!=0x48 || p[1]!=0xb8 || p[10]!=0xff || p[11]!=0xe0)return false;
            for(unsigned i=12;i<18;++i)if(p[i]!=0x90)return false;
            return true;
        }
        Module* Acquire(std::uintptr_t callback,bool creative)
        {
            if(moduleCount==modules.size()){OriginalOwnerFailure::Record("graph-acquire","module-limit");return nullptr;}
            auto& m=modules[moduleCount++];m.callback=callback;
            OriginalOwnerFailure::Module(creative?3:0,0,callback,callback,moduleCount);
            if(!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,reinterpret_cast<LPCWSTR>(callback),&m.reference))
            {OriginalOwnerFailure::Record("graph-acquire","module-reference");ForgetIdentity();return nullptr;}
            m.base=reinterpret_cast<std::uintptr_t>(m.reference);
            for(unsigned i=0;i+1<moduleCount;++i)if(modules[i].base==m.base){OriginalOwnerFailure::Record("graph-acquire","duplicate-module");return nullptr;}
            auto& dos=m.mapped.dos;auto& nt=m.mapped.nt;
            if(!Read(m.base,&dos,sizeof(dos),false) || dos.e_magic!=IMAGE_DOS_SIGNATURE || dos.e_lfanew<=0 ||
                static_cast<std::size_t>(dos.e_lfanew)>4096-sizeof(nt) ||
                !Read(m.base+dos.e_lfanew,&nt,sizeof(nt),false) || nt.Signature!=IMAGE_NT_SIGNATURE ||
                nt.FileHeader.Machine!=IMAGE_FILE_MACHINE_AMD64 || nt.OptionalHeader.Magic!=IMAGE_NT_OPTIONAL_HDR64_MAGIC){OriginalOwnerFailure::Record("graph-acquire","mapped-header");return nullptr;}
            if(creative)m.profile=SelectCreativeProfile(m.base,callback,nt.FileHeader.TimeDateStamp);
            else for(const auto* p:{&Find112,&Find125,&Find128,&Find1210,&Teleport116,&Teleport117,&Teleport1110,&Teleport1111})
                if(callback>=m.base && callback-m.base==p->callback && nt.OptionalHeader.SizeOfImage==p->imageSize){m.profile=p;break;}
            if(!m.profile || !Range(m.base,m.profile->imageSize) || nt.OptionalHeader.SizeOfImage!=m.profile->imageSize){OriginalOwnerFailure::Record("graph-acquire","profile-or-image");return nullptr;}
            if(creative){if(stats.greenCount && !SupportsFiniteCreativeProfile(m.profile)){OriginalOwnerFailure::Record("graph-acquire","creative-profile-after-green");return nullptr;}m.cache=&fileCache;}
            else
            {
                m.family=(m.profile==&Find112 || m.profile==&Find125 || m.profile==&Find128 || m.profile==&Find1210)?1:2;
                if(families&m.family || (requireNewGreen && !New(m.profile))){OriginalOwnerFailure::Record("graph-acquire","family-or-generation");return nullptr;}
                families|=m.family;allGreenNew=allGreenNew&&New(m.profile);
                m.cache=m.family==1?&findFileCache:&teleportFileCache;
            }
            OriginalOwnerFailure::Module(creative?3:m.family,
                m.profile==&Creative320?320:m.profile==&Find1210?1210:m.profile==&Teleport1111?1111:0,m.base,callback,moduleCount);
            const Profile* actual=nullptr;
            if(!FileIdentity(m.reference,m.identity,actual,*m.cache,m.profile,&m.expected,true,&m.deferred) && !m.deferred){OriginalOwnerFailure::Record("graph-acquire","identity");return nullptr;}
            if(!m.deferred && actual!=m.profile){OriginalOwnerFailure::Record("graph-acquire","identity-profile");return nullptr;}
            if(!Read(m.base+m.profile->body,m.mapped.body.data(),m.profile->bodySize)){OriginalOwnerFailure::Record("graph-acquire","mapped-body");return nullptr;}
            if(creative && (!Read(m.base+m.profile->pattern,m.mapped.pattern.data(),m.mapped.pattern.size(),false) ||
                !Read(callback,m.mapped.thunk.data(),m.mapped.thunk.size()) || m.mapped.thunk!=m.profile->thunk)){OriginalOwnerFailure::Record("graph-acquire","creative-pattern-or-thunk");return nullptr;}
            return &m;
        }
        bool Approve(bool allowCold)
        {
            // ColdFileIdentity is unchanged. Reserve its fixed mapped-image
            // Copy calls (DOS/NT/body, plus Creative pattern/thunk) before ANY
            // file attempt, so they share this observation's read ceiling.
            unsigned reserved=0;
            for(unsigned i=0;i<moduleCount;++i)if(modules[i].deferred)
            {
                if(!allowCold)return OriginalOwnerFailure::Refuse("graph-approve","cold-not-permitted");
                reserved+=modules[i].profile->pattern?5:3;
            }
            if(stats.reads>limits.reads || reserved>64-stats.reads || reserved>limits.reads-stats.reads)return OriginalOwnerFailure::Refuse("graph-approve","cold-read-budget");
            stats.reads+=reserved;
            for(unsigned i=0;i<moduleCount;++i)
            {
                auto& m=modules[i];const Profile* actual=nullptr;
                OriginalOwnerFailure::Module(m.family?m.family:3,m.profile==&Creative320?320:m.profile==&Find1210?1210:m.profile==&Teleport1111?1111:0,m.base,m.callback,i+1);
                if(m.deferred && (!allowCold || !FileIdentity(m.reference,m.identity,actual,*m.cache,m.profile,&m.expected)))return OriginalOwnerFailure::Refuse("graph-approve","identity");
                if(m.deferred && actual!=m.profile)return OriginalOwnerFailure::Refuse("graph-approve","identity-profile");
                if(std::memcmp(&m.mapped.dos,&m.expected.dos,sizeof(m.mapped.dos)) ||
                    std::memcmp(&m.mapped.nt,&m.expected.nt,sizeof(m.mapped.nt)) ||
                    std::memcmp(m.mapped.body.data(),m.expected.body.data(),m.profile->bodySize) ||
                    (m.profile->pattern && (m.mapped.pattern!=m.expected.pattern || m.mapped.thunk!=m.expected.thunk)))
                {OriginalOwnerFailure::Record("graph-approve","mapped-identity-changed");ForgetIdentity();return false;}
                if(!Add(m.base) || !Add(m.callback) || !Add(m.identity) || !Add(m.mapped.dos) || !Add(m.mapped.nt) ||
                    !Add(m.mapped.body.data(),m.profile->bodySize) ||
                    (m.profile->pattern && (!Add(m.mapped.pattern) || !Add(m.mapped.thunk))))return OriginalOwnerFailure::Refuse("graph-approve","proof-buffer");
            }
            return OriginalOwnerFailure::Result(IdentityStampValid(stamp),"graph-approve","epoch-after-approval");
        }
        bool Native(std::uintptr_t at,const Profile* source)
        {
            std::array<unsigned char,32> data{};constexpr unsigned char jump[6]={0xff,0x25,0,0,0,0};
            if(!Visit(at) || !Read(at,data.data(),data.size()) || std::memcmp(data.data(),Original,18))return OriginalOwnerFailure::Refuse("graph-native","prefix");
            std::uintptr_t resume=0;
            if(!std::memcmp(data.data()+18,jump,6))resume=Word<std::uintptr_t>(data.data(),24);
            else if(source==&Find112 && data[18]==0xe9)
            {if(!Relative(at+18,Word<std::int32_t>(data.data(),19),resume))return OriginalOwnerFailure::Refuse("graph-native","relative-continuation");}
            else return OriginalOwnerFailure::Refuse("graph-native","continuation-shape");
            const unsigned tag=4;complete=resume==renderer+18;
            if(!complete)return OriginalOwnerFailure::Refuse("graph-native","continuation-target");
            return Add(tag) && Add(at) && Add(data);
        }
        bool Green(std::uintptr_t at,unsigned entryMode=0,bool incomingNear=false)
        {
            if(stats.greenCount==2 || !Visit(at))return OriginalOwnerFailure::Refuse("graph-green","node-or-count");
            std::array<unsigned char,32> relay{};constexpr unsigned char jump[6]={0xff,0x25,10,0,0,0};
            if(!Read(at,relay.data(),relay.size()) || std::memcmp(relay.data(),jump,6))return OriginalOwnerFailure::Refuse("graph-green","relay-prefix");
            auto* m=Acquire(Word<std::uintptr_t>(relay.data(),16),false);if(!m)return OriginalOwnerFailure::Refuse("graph-green","module");
            if((entryMode || incomingNear) && !New(m->profile))return OriginalOwnerFailure::Refuse("graph-green","generation-after-edge");
            if(!New(m->profile))for(unsigned i=6;i<16;++i)if(relay[i])return OriginalOwnerFailure::Refuse("graph-green","legacy-padding");
            ++stats.greenCount;std::uintptr_t next=0;
            if(!Read(m->base+m->profile->slot,&next,sizeof(next),false))return OriginalOwnerFailure::Refuse("graph-green","continuation-slot");
            const unsigned tag=1;
            if(!Add(tag) || !Add(m->family) || !Add(at) || !Add(relay) || !Add(next))return OriginalOwnerFailure::Refuse("graph-green","proof-buffer");
            return Edge(next,m->profile);
        }
        bool Creative(std::uintptr_t block,unsigned kind,std::uintptr_t relayAddress=0)
        {
            if(stats.creativeCount || (stats.greenCount && !allGreenNew) || !Visit(block))return OriginalOwnerFailure::Refuse("graph-creative","node-count-or-order");
            ++stats.creativeCount;std::array<unsigned char,BlockSize> code{};
            if(!Read(block,code.data(),code.size()))return OriginalOwnerFailure::Refuse("graph-creative","block-read");
            auto* m=Acquire(Word<std::uintptr_t>(code.data(),0x177),true);if(!m)return OriginalOwnerFailure::Refuse("graph-creative","module");
            std::uintptr_t object=0;std::array<unsigned char,0xd8> model{};
            if(!Read(m->base+m->profile->slot,&object,sizeof(object),false) || (object&7) || !Range(object,m->profile->objectSize) ||
                !Read(object+0x80,model.data(),model.size(),false))return OriginalOwnerFailure::Refuse("graph-creative","object-layout");
            const auto get=[&](std::size_t n){return Word<std::uintptr_t>(model.data(),n);};
            const auto t=get(0x20),length=get(0x28),backup=get(0x30);
            const std::size_t n=kind==2?18:length==29?10:length==30?11:0;
            const auto bad=[](bool failed,const char* reason){return OriginalOwnerFailure::RejectIf(failed,"graph-creative",reason);};
            if(bad(!n,"stolen-length") || bad(kind!=1 && kind!=2,"mode") || bad(length!=(kind==2?32:n+19),"allocation-length") ||
                bad(get(0)!=renderer,"target") || bad(get(8)!=block,"block") || bad(!Range(get(0x10),1),"allocation-owner") ||
                bad(!Range(get(0x18),1),"allocation-control") || bad(!Range(backup,n),"saved-range") || bad(get(0x38)!=backup+n,"saved-end") ||
                bad(get(0x40)<get(0x38),"saved-capacity-before-end") || bad(get(0x40)-backup>64,"saved-capacity-limit") ||
                bad(get(0x48)!=length,"emitted-length") || bad(model[0xa0]!=1,"hook-inactive") ||
                bad(Word<unsigned>(model.data(),0xa4)!=kind,"hook-mode") || bad(get(0xa8)!=renderer,"secondary-target") ||
                bad(!Range(get(0xb0),1),"secondary-owner") || bad(!Range(get(0xb8),1),"secondary-control") ||
                bad(get(0xc0)!=block,"secondary-block") || bad(get(0xc8)!=BlockSize,"block-size") || bad(get(0xd0)!=m->callback,"callback") ||
                bad(Word<std::uintptr_t>(code.data(),0x17f)!=t,"continuation-cell") || bad(!Visit(t),"continuation-node"))
                return OriginalOwnerFailure::Refuse("graph-creative","hook-model");
            std::array<unsigned char,32> trampoline{};std::array<unsigned char,18> saved{};
            if(!Read(t,trampoline.data(),length) || !Read(backup,saved.data(),n,false) || std::memcmp(saved.data(),trampoline.data(),n))return OriginalOwnerFailure::Refuse("graph-creative","saved-prefix");
            constexpr unsigned char indirect[6]={0xff,0x25,0,0,0,0};
            if(kind==1)
            {
                std::uintptr_t resume=0;
                if(!Range(t,length) || relayAddress!=t+n+5 || !Visit(relayAddress) || trampoline[n]!=0xe9 ||
                    !Relative(t+n,Word<std::int32_t>(trampoline.data(),n+1),resume) || resume!=renderer+n ||
                    std::memcmp(trampoline.data()+n+5,indirect,6) || Word<std::uintptr_t>(trampoline.data(),n+11)!=block)return OriginalOwnerFailure::Refuse("graph-creative","near-continuation");
                for(unsigned i=5;i<n;++i)if(live[i]!=0x90)return OriginalOwnerFailure::Refuse("graph-creative","entry-padding");
                std::memcpy(saved.data()+n,live.data()+n,18-n);
            }
            else if(std::memcmp(trampoline.data()+18,indirect,6) || Word<std::uintptr_t>(trampoline.data(),24)!=renderer+18)return OriginalOwnerFailure::Refuse("graph-creative","far-continuation");
            auto normalized=code;std::memset(normalized.data()+0x177,0,16);
            if(normalized!=m->mapped.pattern)return OriginalOwnerFailure::Refuse("graph-creative","emitter-template");
            const bool native=!std::memcmp(saved.data(),Original,18),mov=Mov(saved.data());
            if((n==11 && !native) || (n==10 && !mov) || (!native && !mov) ||
                (kind==2 && mov && !SupportsFiniteCreativeProfile(m->profile)))return OriginalOwnerFailure::Refuse("graph-creative","saved-entry-shape");
            if(!stats.greenCount)rootKind=kind;
            const unsigned tag=2;
            if(!Add(tag) || !Add(kind) || !Add(n) || !Add(block) || !Add(t) || !Add(object) || !Add(relayAddress) ||
                !Add(code) || !Add(model) || !Add(saved) || !Add(trampoline) || !Add(backup))return OriginalOwnerFailure::Refuse("graph-creative","proof-buffer");
            if(native){complete=true;return true;}
            if(stats.greenCount || kind==2)requireNewGreen=true;
            return Green(Word<std::uintptr_t>(saved.data(),2));
        }
        bool Indirect(std::uintptr_t at,const Profile* source,unsigned rootEntryMode=0)
        {
            constexpr unsigned char indirect[6]={0xff,0x25,0,0,0,0},generic[6]={0xff,0x25,10,0,0,0};
            std::array<unsigned char,14> relay{};std::array<unsigned char,6> prefix{};
            if(!Read(at,relay.data(),relay.size()) || std::memcmp(relay.data(),indirect,6))return OriginalOwnerFailure::Refuse("graph-indirect","relay-prefix");
            const auto destination=Word<std::uintptr_t>(relay.data(),6);
            if(!Read(destination,prefix.data(),prefix.size()))return OriginalOwnerFailure::Refuse("graph-indirect","destination-read");
            if(!std::memcmp(prefix.data(),generic,6))
            {
                if((source && !New(source)) || stats.nearCount==2 || !Visit(at))return OriginalOwnerFailure::Refuse("graph-indirect","generation-count-or-node");
                ++stats.nearCount;const unsigned tag=3;
                if(!Add(tag) || !Add(at) || !Add(relay))return OriginalOwnerFailure::Refuse("graph-indirect","proof-buffer");
                return Green(destination,rootEntryMode,true);
            }
            // A second known near hop is not a Creative block. Do not read
            // beyond its defined14B while refusing the unsupported shape.
            if(!std::memcmp(prefix.data(),indirect,6))return OriginalOwnerFailure::Refuse("graph-indirect","second-indirect-hop");
            if(source && (!New(source) || rootMode!=1))return OriginalOwnerFailure::Refuse("graph-indirect","creative-edge-generation");
            return Creative(destination,1,at);
        }
        bool Edge(std::uintptr_t at,const Profile* source)
        {
            std::array<unsigned char,6> prefix{};
            if(!Read(at,prefix.data(),prefix.size()))return OriginalOwnerFailure::Refuse("graph-edge","prefix-read");
            constexpr unsigned char generic[6]={0xff,0x25,10,0,0,0},indirect[6]={0xff,0x25,0,0,0,0};
            if(!std::memcmp(prefix.data(),Original,6))return Native(at,source);
            if(!std::memcmp(prefix.data(),generic,6))return Green(at);
            if(!std::memcmp(prefix.data(),indirect,6))return Indirect(at,source);
            if(New(source) && rootMode==2)return Creative(at,2);
            return OriginalOwnerFailure::Refuse("graph-edge","unknown-edge-shape");
        }
        bool Capture(std::uintptr_t entry,bool allowCold)
        {
            renderer=entry;if(!Range(entry,18) || !Read(entry,live.data(),live.size()))return OriginalOwnerFailure::Refuse("graph-capture","entry-range-or-read");
            const unsigned tag=0;if(!Add(tag) || !Add(entry) || !Add(live))return OriginalOwnerFailure::Refuse("graph-capture","proof-buffer");
            bool ok=false;constexpr unsigned char indirect[6]={0xff,0x25,0,0,0,0},generic[6]={0xff,0x25,10,0,0,0};
            if(Mov(live.data()))ok=Green(Word<std::uintptr_t>(live.data(),2));
            else if(live[0]==0xe9)
            {
                rootMode=1;std::uintptr_t at=0;
                if(!Relative(entry,Word<std::int32_t>(live.data(),1),at))return OriginalOwnerFailure::Refuse("graph-capture","relative-entry");
                ok=Indirect(at,nullptr,1);
            }
            else if(!std::memcmp(live.data(),indirect,6))
            {
                rootMode=2;for(unsigned i=14;i<18;++i)if(live[i]!=0x90)return OriginalOwnerFailure::Refuse("graph-capture","entry-padding");
                const auto at=Word<std::uintptr_t>(live.data(),6);std::array<unsigned char,6> prefix{};
                if(!Read(at,prefix.data(),prefix.size()))return OriginalOwnerFailure::Refuse("graph-capture","root-destination-read");
                if(!std::memcmp(prefix.data(),indirect,6))return OriginalOwnerFailure::Refuse("graph-capture","root-indirect-hop");
                ok=!std::memcmp(prefix.data(),generic,6)?Green(at,2):Creative(at,2);
            }
            if(!ok)return OriginalOwnerFailure::Refuse("graph-capture","entry-shape-or-edge");
            if(!complete)return OriginalOwnerFailure::Refuse("graph-capture","incomplete-chain");
            if(!Approve(allowCold))return OriginalOwnerFailure::Refuse("graph-capture","approval");
            return OriginalOwnerFailure::Result(IdentityStampValid(stamp),"graph-capture","epoch-after-capture");
        }
    };
    // Private SF lifetime handoff only. Stores approved module identity/lifetime,
    // never a foreign graph or an opaque cache-key-derived pointer. References
    // intentionally survive logical Stop and are retained until process exit.
    class ApprovedProviderLeases final
    {
        struct Lease { HMODULE reference{};std::uintptr_t base{},callback{};const Profile* profile{};Digest identity{}; };
        std::array<Lease,3> leases_{};unsigned count_{};bool attempted_{},expansion_attempted_{};IdentityStamp stamp_{};
        static bool Same(const Module& a,const Module& b) noexcept {
            return a.reference==b.reference && a.base==b.base && a.callback==b.callback && a.profile==b.profile && a.identity==b.identity;
        }
        static bool CurrentCohort(const Observation& observed) noexcept {
            if(observed.moduleCount!=3)return OriginalOwnerFailure::Refuse("lease-cohort","observation-module-count");
            bool creative{},find{},teleport{};
            for(unsigned i=0;i<3;++i){const auto* p=observed.modules[i].profile;
                if(p==&Creative320 && !creative)creative=true;
                else if(p==&Find1210 && !find)find=true;
                else if(p==&Teleport1111 && !teleport)teleport=true;
                else return OriginalOwnerFailure::Refuse("lease-cohort","observation-profile");
            }return OriginalOwnerFailure::Result(creative && find && teleport,"lease-cohort","observation-missing-profile");
        }
        bool Expand(const Observation& before,const Observation& after,Diagnostic* diagnostic) noexcept {
            // One explicit post-scan expansion for the selected current cohort.
            // Existing Creative residency is retained; callbacks never acquire.
            if(expansion_attempted_)return OriginalOwnerFailure::Refuse("lease-expand","already-attempted");
            if(count_!=1)return OriginalOwnerFailure::Refuse("lease-expand","retained-count");
            if(leases_[0].profile!=&Creative320)return OriginalOwnerFailure::Refuse("lease-expand","retained-profile");
            if(coldAdmissionEpoch!=stamp_.epoch)return OriginalOwnerFailure::Refuse("lease-expand","cold-epoch");
            if(identityOrigin!=ProofOrigin::AdmissionUpdate)return OriginalOwnerFailure::Refuse("lease-expand","not-control-plane");
            if(!IdentityStampValid(stamp_))return OriginalOwnerFailure::Refuse("lease-expand","retained-epoch-invalid");
            if(before.stamp.epoch!=stamp_.epoch)return OriginalOwnerFailure::Refuse("lease-expand","before-epoch");
            if(after.stamp.epoch!=stamp_.epoch)return OriginalOwnerFailure::Refuse("lease-expand","after-epoch");
            if(before.stamp.invalidation!=stamp_.invalidation)return OriginalOwnerFailure::Refuse("lease-expand","before-invalidation");
            if(after.stamp.invalidation!=stamp_.invalidation)return OriginalOwnerFailure::Refuse("lease-expand","after-invalidation");
            if(!CurrentCohort(before) || !CurrentCohort(after))return OriginalOwnerFailure::Refuse("lease-expand","cohort");
            expansion_attempted_=true;
            std::array<Lease,3> pending{};std::array<bool,3> acquired{};bool found_old{},valid=true;
            for(unsigned i=0;i<3;++i){const auto& b=before.modules[i];const auto& a=after.modules[i];
                OriginalOwnerFailure::Module(b.profile==&Creative320?3:b.profile==&Find1210?1:2,
                    b.profile==&Creative320?320:b.profile==&Find1210?1210:b.profile==&Teleport1111?1111:0,b.base,b.callback,i+1);
                if(!Same(b,a)){OriginalOwnerFailure::Record("lease-expand","observation-changed");valid=false;break;}
                auto& next=pending[i];
                if(b.profile==&Creative320){
                    const auto& old=leases_[0];
                    if(old.reference!=b.reference || old.base!=b.base || old.callback!=b.callback || old.identity!=b.identity){OriginalOwnerFailure::Record("lease-expand","old-creative-changed");valid=false;break;}
                    next=old;found_old=true;
                }else{
                    if(!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,reinterpret_cast<LPCWSTR>(b.callback),&next.reference)) {OriginalOwnerFailure::Record("lease-expand","reference-acquire");valid=false;break;}
                    acquired[i]=true;
                    if(next.reference!=b.reference){OriginalOwnerFailure::Record("lease-expand","reference-base");valid=false;break;}
                    next.base=b.base;next.callback=b.callback;next.profile=b.profile;next.identity=b.identity;
                }
            }
            if(!valid || !found_old || !IdentityStampValid(stamp_)){
                OriginalOwnerFailure::Record("lease-expand",!valid?"acquisition-refused":!found_old?"old-creative-missing":"epoch-before-publish");
                for(unsigned i=0;i<3;++i)if(acquired[i])FreeLibrary(pending[i].reference);
                return false;
            }
            leases_=pending;count_=3;
            if(diagnostic){diagnostic->retainedModules=count_;diagnostic->reason="none";}
            return true;
        }
        bool Matches(const Observation& observed) const noexcept {
            if(!count_ || count_!=observed.moduleCount || stamp_.epoch!=observed.stamp.epoch ||
               stamp_.invalidation!=observed.stamp.invalidation || !IdentityStampValid(stamp_))return false;
            for(unsigned i=0;i<count_;++i){const auto& l=leases_[i];const auto& m=observed.modules[i];
                if(l.reference!=m.reference || l.base!=m.base || l.callback!=m.callback ||
                   l.profile!=m.profile || l.identity!=m.identity)return false;
            }return true;
        }
    public:
        ApprovedProviderLeases()=default;
        ApprovedProviderLeases(const ApprovedProviderLeases&)=delete;
        ApprovedProviderLeases& operator=(const ApprovedProviderLeases&)=delete;
        bool Retained() const noexcept{return count_!=0;}
        bool HasCurrentCohort() const noexcept {
            if(count_!=3)return OriginalOwnerFailure::Refuse("lease-cohort","retained-module-count");
            if(!IdentityStampValid(stamp_))return OriginalOwnerFailure::Refuse("lease-cohort","retained-epoch");
            bool creative{},find{},teleport{};
            for(const auto& lease:leases_){creative|=lease.profile==&Creative320;find|=lease.profile==&Find1210;teleport|=lease.profile==&Teleport1111;}
            return OriginalOwnerFailure::Result(creative && find && teleport,"lease-cohort","retained-profile");
        }
        bool Accept(const Observation& before,const Observation& after,bool allowAcquire,Diagnostic* diagnostic=nullptr) noexcept {
            if(diagnostic){diagnostic->retainedModules=count_;diagnostic->reason="lease-refused";}
            if(count_){
                const bool valid=Matches(before)&&Matches(after); // no extra persistent reference
                if(diagnostic)diagnostic->reason=valid?"none":
                    (count_!=before.moduleCount || count_!=after.moduleCount)?"lease-module-count":"lease-identity-or-epoch";
                const bool accepted=valid || (allowAcquire && Expand(before,after,diagnostic));
                if(!accepted)OriginalOwnerFailure::Record("lease","retained-proof-refused");
                return accepted;
            }
            if(!allowAcquire)return OriginalOwnerFailure::Refuse("lease-acquire","not-allowed");
            if(attempted_)return OriginalOwnerFailure::Refuse("lease-acquire","attempt-consumed");
            if(!before.moduleCount)return OriginalOwnerFailure::Refuse("lease-acquire","empty-observation");
            if(before.moduleCount>leases_.size())return OriginalOwnerFailure::Refuse("lease-acquire","module-limit");
            if(before.moduleCount!=after.moduleCount)return OriginalOwnerFailure::Refuse("lease-acquire","module-count-changed");
            if(coldAdmissionEpoch!=before.stamp.epoch)return OriginalOwnerFailure::Refuse("lease-acquire","cold-epoch");
            if(identityOrigin!=ProofOrigin::AdmissionUpdate)return OriginalOwnerFailure::Refuse("lease-acquire","not-control-plane");
            if(!IdentityStampValid(before.stamp))return OriginalOwnerFailure::Refuse("lease-acquire","epoch-before-acquire");
            attempted_=true;std::array<Lease,3> pending{};bool valid=true;
            for(unsigned i=0;i<before.moduleCount;++i){const auto& b=before.modules[i];const auto& a=after.modules[i];
                if(b.reference!=a.reference || b.base!=a.base || b.callback!=a.callback || b.profile!=a.profile || b.identity!=a.identity ||
                   before.stamp.epoch!=after.stamp.epoch || before.stamp.invalidation!=after.stamp.invalidation){OriginalOwnerFailure::Record("lease-acquire","observation-changed");valid=false;break;}
                auto& l=pending[i];
                if(!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,reinterpret_cast<LPCWSTR>(b.callback),&l.reference) ||
                   l.reference!=b.reference){OriginalOwnerFailure::Record("lease-acquire","reference-acquire-or-base");valid=false;break;}
                l.base=b.base;l.callback=b.callback;l.profile=b.profile;l.identity=b.identity;
            }
            valid=valid && OriginalOwnerFailure::Result(IdentityStampValid(before.stamp),"lease-acquire","before-epoch-invalid") &&
                OriginalOwnerFailure::Result(IdentityStampValid(after.stamp),"lease-acquire","after-epoch-invalid");
            if(!valid){for(auto& l:pending)if(l.reference)FreeLibrary(l.reference);return false;}
            // Both complete Observation reference sets still exist at this point.
            leases_=pending;count_=before.moduleCount;stamp_=before.stamp;
            if(diagnostic){diagnostic->retainedModules=count_;diagnostic->reason="none";}return true;
        }
    };
    inline bool Inspect(std::uintptr_t entry,Proof& proof,Limits limits={},Stats* stats=nullptr,
        ApprovedProviderLeases* leases=nullptr,bool acquireLeases=false,Diagnostic* diagnostic=nullptr)
    {
        OriginalOwnerFailure::ContextScope diagnosticContext;
        proof={};Observation before(limits),after(limits);
        if(diagnostic)*diagnostic={};
        const bool capturedBefore=before.Capture(entry,true);
        const bool capturedAfter=capturedBefore && after.Capture(entry,false);
        bool ok=capturedAfter &&
            before.rootKind==after.rootKind && before.used==after.used &&
            before.stamp.epoch==after.stamp.epoch && before.stamp.invalidation==after.stamp.invalidation &&
            IdentityStampValid(before.stamp) && !std::memcmp(before.bytes.data(),after.bytes.data(),before.used);
        if(stats){*stats=before.stats;stats->reads=before.stats.reads>after.stats.reads?before.stats.reads:after.stats.reads;}
        if(diagnostic){diagnostic->beforeModules=before.moduleCount;diagnostic->afterModules=after.moduleCount;
            diagnostic->reads=(std::max)(before.stats.reads,after.stats.reads);
            diagnostic->reason=!capturedBefore?"graph-before":!capturedAfter?"graph-after":"graph-changed";}
        OriginalOwnerFailure::Graph(before.moduleCount,after.moduleCount,diagnostic?diagnostic->retainedModules:0,
            (std::max)(before.stats.reads,after.stats.reads),before.stamp.epoch,before.stamp.invalidation,after.stamp.epoch,after.stamp.invalidation);
        if(!ok)return OriginalOwnerFailure::Refuse("graph-compare",!capturedBefore?"before-refused":!capturedAfter?"after-refused":"observation-changed");
        if(diagnostic)diagnostic->reason="graph-digest";
        Hash hash;if(!hash.Add(before.bytes.data(),before.used) || !hash.Finish(proof.digest) || !IdentityStampValid(before.stamp))return OriginalOwnerFailure::Refuse("graph-digest","hash-or-epoch");
        const bool accepted=!leases || leases->Accept(before,after,acquireLeases,diagnostic);
        OriginalOwnerFailure::Graph(before.moduleCount,after.moduleCount,diagnostic?diagnostic->retainedModules:0,
            (std::max)(before.stats.reads,after.stats.reads),before.stamp.epoch,before.stamp.invalidation,after.stamp.epoch,after.stamp.invalidation);
        if(!accepted){proof={};return OriginalOwnerFailure::Refuse("graph-lease","refused");}
        if(diagnostic)diagnostic->reason="none";
        proof.kind=before.rootKind;return true;
    }
}
