// Included inside PublicCreativeCompat. Cold work is explicitly admitted once;
// callbacks only copy owned approvals under a bounded metadata try-lock.
struct TrustedImageBytes
{
    IMAGE_DOS_HEADER dos{};
    IMAGE_NT_HEADERS64 nt{};
    std::array<unsigned char,BodySize> body{};
    std::array<unsigned char,BlockSize> pattern{};
    std::array<unsigned char,5> thunk{};
};
struct IdentityStamp
{
    std::uint64_t epoch=0,invalidation=0;
};
inline std::atomic<std::uint64_t> ownershipEpoch{1},ownershipInvalidation{1};
inline thread_local std::uint64_t coldAdmissionEpoch=0;
inline thread_local ProofOrigin identityOrigin=ProofOrigin::Other;
inline IdentityStamp CurrentIdentityStamp()
{return {ownershipEpoch.load(std::memory_order_acquire),ownershipInvalidation.load(std::memory_order_acquire)};}
inline bool IdentityStampValid(const IdentityStamp& value)
{const auto now=CurrentIdentityStamp();return value.epoch==now.epoch && value.invalidation==now.invalidation;}
class AdmissionEpochScope
{
    std::uint64_t prior;
public:
    AdmissionEpochScope():prior(coldAdmissionEpoch)
    {coldAdmissionEpoch=ownershipEpoch.fetch_add(1,std::memory_order_acq_rel)+1;ownershipInvalidation.fetch_add(1,std::memory_order_acq_rel);}
    ~AdmissionEpochScope(){coldAdmissionEpoch=prior;}
};
class IdentityOriginScope
{
    ProofOrigin priorOrigin;std::uint64_t priorPermit;
public:
    explicit IdentityOriginScope(ProofOrigin origin):priorOrigin(identityOrigin),priorPermit(coldAdmissionEpoch)
    {identityOrigin=origin;if(origin==ProofOrigin::CallbackPre || origin==ProofOrigin::CallbackPost)coldAdmissionEpoch=0;}
    ~IdentityOriginScope(){identityOrigin=priorOrigin;coldAdmissionEpoch=priorPermit;}
};
// The loader's explicit Activate boundary can spend an unused Load-epoch
// attempt. It never replenishes a rejected/in-progress provider's budget.
class ContinueAdmissionScope
{
    std::uint64_t prior;
public:
    ContinueAdmissionScope():prior(coldAdmissionEpoch){coldAdmissionEpoch=ownershipEpoch.load(std::memory_order_acquire);}
    ~ContinueAdmissionScope(){coldAdmissionEpoch=prior;}
};
enum class IdentityState { Unseen, InProgress, Approved, Rejected };
struct FileCache
{
    std::mutex mutex;
    std::uint64_t epoch=0,invalidation=0,reservation=0;
    std::uintptr_t opaqueKey=0;
    bool attempted=false,trusted=false;
    IdentityState state=IdentityState::Unseen;
    const Profile* profile=nullptr; // owned constexpr descriptor only
    std::wstring path;
    BY_HANDLE_FILE_INFORMATION info{};
    Digest imageIdentity{};
    TrustedImageBytes expected{};
};
inline FileCache fileCache,findFileCache,teleportFileCache;
// Invalidation never waits for a cold reader and never replenishes its budget.
inline void ForgetFindIdentity(){ownershipInvalidation.fetch_add(1,std::memory_order_acq_rel);}
inline void ForgetIdentity(){ownershipInvalidation.fetch_add(1,std::memory_order_acq_rel);}
inline bool SameFile(const BY_HANDLE_FILE_INFORMATION& a,const BY_HANDLE_FILE_INFORMATION& b)
{
    return a.dwVolumeSerialNumber==b.dwVolumeSerialNumber && a.nFileIndexHigh==b.nFileIndexHigh &&
        a.nFileIndexLow==b.nFileIndexLow && a.nFileSizeHigh==b.nFileSizeHigh && a.nFileSizeLow==b.nFileSizeLow &&
        a.ftLastWriteTime.dwHighDateTime==b.ftLastWriteTime.dwHighDateTime &&
        a.ftLastWriteTime.dwLowDateTime==b.ftLastWriteTime.dwLowDateTime;
}
// Keep the large file/path buffers out of every warm callback stack frame.
__declspec(noinline) inline bool ColdFileIdentity(HMODULE module,const Profile& profile,std::wstring& path,
    BY_HANDLE_FILE_INFORMATION& info,Digest& identity,TrustedImageBytes& expected)
{
    wchar_t name[32768]{};
    const DWORD length=GetModuleFileNameW(module,name,static_cast<DWORD>(std::size(name)));
    if(!length || length>=std::size(name))return OriginalOwnerFailure::Refuse("file-cold","module-path");
    path.assign(name,length);
#if ESM_AGGREGATE_TIMING_DIAGNOSTICS
    auto& collector=MinimapAggregateTiming::DiagnosticCollector();
#endif
    MINIMAP_PCC_AGG_COUNT(collector,MinimapAggregateTiming::Metric::BackingFileOpenAttempt);
    const HANDLE file=CreateFileW(name,GENERIC_READ,FILE_SHARE_READ|FILE_SHARE_DELETE,nullptr,OPEN_EXISTING,FILE_ATTRIBUTE_NORMAL,nullptr);
    if(file==INVALID_HANDLE_VALUE)
    {MINIMAP_PCC_AGG_COUNT(collector,MinimapAggregateTiming::Metric::BackingFileOpenFailure);return OriginalOwnerFailure::Refuse("file-cold","file-open");}
    MINIMAP_PCC_AGG_COUNT(collector,MinimapAggregateTiming::Metric::BackingFileOpenSuccess);
    struct FileReference{HANDLE value;~FileReference(){CloseHandle(value);}} reference{file};
    BY_HANDLE_FILE_INFORMATION before{},after{};
    if(!GetFileInformationByHandle(file,&before) || before.nFileSizeHigh || before.nFileSizeLow!=profile.fileSize)return OriginalOwnerFailure::Refuse("file-cold","file-info-or-size");
    Hash hash;std::array<unsigned char,65536> buffer{};std::size_t total=0;
    while(total<profile.fileSize)
    {
        DWORD read=0;const DWORD wanted=static_cast<DWORD>((std::min)(profile.fileSize-total,buffer.size()));
#if ESM_AGGREGATE_TIMING_DIAGNOSTICS
        std::uint64_t start=0,end=0;const bool clock=ReadProofQpc(start);
#endif
        const BOOL ok=ReadFile(file,buffer.data(),wanted,&read,nullptr);
#if ESM_AGGREGATE_TIMING_DIAGNOSTICS
        if(clock && ReadProofQpc(end) && end>=start)MINIMAP_PCC_AGG_RECORD(collector,MinimapAggregateTiming::Metric::BackingFileRead,end-start,read);
        else MINIMAP_PCC_AGG_TIMING_FAILURE(collector,MinimapAggregateTiming::Metric::BackingFileRead);
#endif
        if(!ok)return OriginalOwnerFailure::Refuse("file-cold","file-read");
        if(read!=wanted)return OriginalOwnerFailure::Refuse("file-cold","short-file-read");
        if(!hash.Add(buffer.data(),read))return OriginalOwnerFailure::Refuse("file-cold","hash-update");
        total+=read;
    }
    Digest digest{};
    if(!hash.Finish(digest))return OriginalOwnerFailure::Refuse("file-cold","hash-finish");
    if(!HexEquals(digest,profile.fileHash))return OriginalOwnerFailure::Refuse("file-cold","file-hash");
    if(!GetFileInformationByHandle(file,&after))return OriginalOwnerFailure::Refuse("file-cold","file-info-after");
    if(!SameFile(before,after))return OriginalOwnerFailure::Refuse("file-cold","file-changed");
    const auto base=reinterpret_cast<std::uintptr_t>(module);
    if(!Copy(base,&expected.dos,sizeof(expected.dos)))return OriginalOwnerFailure::Refuse("file-cold","mapped-dos-read");
    if(expected.dos.e_magic!=IMAGE_DOS_SIGNATURE || expected.dos.e_lfanew<=0 ||
        static_cast<std::size_t>(expected.dos.e_lfanew)>4096-sizeof(expected.nt))return OriginalOwnerFailure::Refuse("file-cold","mapped-dos");
    if(!Copy(base+expected.dos.e_lfanew,&expected.nt,sizeof(expected.nt)))return OriginalOwnerFailure::Refuse("file-cold","mapped-nt-read");
    if(expected.nt.Signature!=IMAGE_NT_SIGNATURE || expected.nt.FileHeader.Machine!=IMAGE_FILE_MACHINE_AMD64 ||
        expected.nt.OptionalHeader.Magic!=IMAGE_NT_OPTIONAL_HDR64_MAGIC || expected.nt.OptionalHeader.SizeOfImage!=profile.imageSize)
        return OriginalOwnerFailure::Refuse("file-cold","mapped-nt");
    if(!Copy(base+profile.body,expected.body.data(),profile.bodySize,true))return OriginalOwnerFailure::Refuse("file-cold","mapped-body-read");
    if(!HasHash(expected.body.data(),profile.bodySize,profile.bodyHash))return OriginalOwnerFailure::Refuse("file-cold","mapped-body-hash");
    if(profile.pattern) {
        if(!Copy(base+profile.pattern,expected.pattern.data(),expected.pattern.size()))return OriginalOwnerFailure::Refuse("file-cold","mapped-pattern-read");
        if(!HasHash(expected.pattern.data(),expected.pattern.size(),TemplateHash))return OriginalOwnerFailure::Refuse("file-cold","mapped-pattern-hash");
        if(!Copy(base+profile.callback,expected.thunk.data(),expected.thunk.size(),true))return OriginalOwnerFailure::Refuse("file-cold","mapped-thunk-read");
        if(expected.thunk!=profile.thunk)return OriginalOwnerFailure::Refuse("file-cold","mapped-thunk");
    }
    identity=digest;info=after;return true;
}
inline bool FileIdentity(HMODULE module,Digest& identity,const Profile*& profile,
    FileCache& cache=fileCache,const Profile* required=nullptr,TrustedImageBytes* expected=nullptr,
    bool deferCold=false,bool* deferred=nullptr)
{
    profile=nullptr;identity={};
    if(deferred)*deferred=false;
    if(!module || !required)return OriginalOwnerFailure::Refuse("file-identity","module-or-profile");
    // Fresh referenced module ownership and approved mapped bytes establish
    // affinity. Path discovery belongs only to the consumed cold attempt.
    // Unobserved same-base unload/reload ABA still requires a process restart.
    const auto stamp=CurrentIdentityStamp();
    // A collision-free opaque equality tag, never dereferenced or used as trust.
    const auto key=reinterpret_cast<std::uintptr_t>(module)^reinterpret_cast<std::uintptr_t>(&ownershipEpoch);
    std::uint64_t reservation=0;
    {
        std::unique_lock<std::mutex> lock(cache.mutex,std::try_to_lock);
        if(!lock)return OriginalOwnerFailure::Refuse("file-identity","cache-busy");
        if(!IdentityStampValid(stamp))return OriginalOwnerFailure::Refuse("file-identity","epoch-before-cache");
        const bool cold=coldAdmissionEpoch==stamp.epoch && identityOrigin!=ProofOrigin::CallbackPre && identityOrigin!=ProofOrigin::CallbackPost;
        if(cache.epoch!=stamp.epoch)
        {
            if(!cold)return OriginalOwnerFailure::Refuse("file-identity","no-cold-epoch");
            cache.epoch=stamp.epoch;cache.invalidation=stamp.invalidation;cache.attempted=false;cache.trusted=false;
            cache.state=IdentityState::Unseen;cache.profile=required;cache.opaqueKey=key;++cache.reservation;
        }
        if(cache.profile!=required || cache.opaqueKey!=key || cache.invalidation!=stamp.invalidation)
        {
            OriginalOwnerFailure::Record("file-identity","cache-binding-or-invalidation");
            if(cache.profile!=required || cache.opaqueKey!=key)ForgetIdentity();
            cache.trusted=false;cache.state=IdentityState::Rejected;++cache.reservation;
            return false; // observed replacement/invalidation requires an explicit new epoch
        }
        if(cache.state==IdentityState::Approved && cache.trusted)
        {
            identity=cache.imageIdentity;profile=cache.profile;if(expected)*expected=cache.expected;
#if ESM_AGGREGATE_TIMING_DIAGNOSTICS
            MINIMAP_PCC_AGG_COUNT(MinimapAggregateTiming::DiagnosticCollector(),MinimapAggregateTiming::Metric::ProofCacheWarmHit);
#endif
            return OriginalOwnerFailure::Result(IdentityStampValid(stamp),"file-identity","epoch-after-copy");
        }
        if(!cold)return OriginalOwnerFailure::Refuse("file-identity","cold-not-permitted");
        if(cache.attempted)return OriginalOwnerFailure::Refuse("file-identity","cold-attempt-consumed");
        if(cache.state!=IdentityState::Unseen)return OriginalOwnerFailure::Refuse("file-identity","cache-not-unseen");
        // A chain first validates its bounded topology without file work.
        if(deferCold){if(deferred)*deferred=true;return false;}
        cache.attempted=true;cache.state=IdentityState::InProgress;reservation=++cache.reservation;
        MINIMAP_PCC_AGG_COUNT(MinimapAggregateTiming::DiagnosticCollector(),MinimapAggregateTiming::Metric::ProofCacheColdAttempt);
    }
    // No cache lock, callback wait, or partial approved payload exists during I/O.
    std::wstring path;BY_HANDLE_FILE_INFORMATION info{};Digest result{};TrustedImageBytes bytes{};
#if ESM_AGGREGATE_TIMING_DIAGNOSTICS
    struct ColdOutcome
    {
        bool success=false;
        ~ColdOutcome(){MinimapAggregateTiming::DiagnosticCollector().Count(success?
            MinimapAggregateTiming::Metric::ProofColdSuccess:MinimapAggregateTiming::Metric::ProofColdFailure);}
    } outcome;
#endif
    bool accepted=false;
    try{accepted=ColdFileIdentity(module,*required,path,info,result,bytes);}
    catch(...){OriginalOwnerFailure::Record("file-cold","exception");accepted=false;} // Allocation/path failures are terminal attempts too.
    {
        std::lock_guard<std::mutex> lock(cache.mutex);
        if(!IdentityStampValid(stamp) || cache.epoch!=stamp.epoch || cache.invalidation!=stamp.invalidation ||
            cache.opaqueKey!=key || cache.profile!=required || cache.reservation!=reservation || cache.state!=IdentityState::InProgress)return OriginalOwnerFailure::Refuse("file-identity","cold-commit-cancelled");
        cache.state=accepted?IdentityState::Approved:IdentityState::Rejected;cache.trusted=accepted;
        if(!accepted)return OriginalOwnerFailure::Refuse("file-cold","cold-refused");
        cache.path=std::move(path);cache.info=info;cache.imageIdentity=result;cache.expected=bytes;
        identity=result;profile=required;if(expected)*expected=bytes;
#if ESM_AGGREGATE_TIMING_DIAGNOSTICS
        outcome.success=true;
#endif
    }
    return OriginalOwnerFailure::Result(IdentityStampValid(stamp),"file-identity","epoch-after-copy");
}
