pub const MDBX_VERSION_MAJOR: ::libc::c_uint = 0;
pub const MDBX_VERSION_MINOR: ::libc::c_uint = 9;
pub const MDBX_LOCKNAME: &'static [u8; 10usize] = b"/mdbx.lck\0";
pub const MDBX_DATANAME: &'static [u8; 10usize] = b"/mdbx.dat\0";
pub const MDBX_LOCK_SUFFIX: &'static [u8; 5usize] = b"-lck\0";
pub type va_list = __builtin_va_list;
pub type __uint16_t = ::libc::c_ushort;
pub type __int32_t = ::libc::c_int;
pub type __darwin_intptr_t = ::libc::c_long;
pub type __darwin_mode_t = __uint16_t;
pub type __darwin_pid_t = __int32_t;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct __darwin_pthread_handler_rec {
    pub __routine: ::std::option::Option<unsafe extern "C" fn(arg1: *mut ::libc::c_void)>,
    pub __arg: *mut ::libc::c_void,
    pub __next: *mut __darwin_pthread_handler_rec,
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct _opaque_pthread_t {
    pub __sig: ::libc::c_long,
    pub __cleanup_stack: *mut __darwin_pthread_handler_rec,
    pub __opaque: [::libc::c_char; 8176usize],
}
pub type __darwin_pthread_t = *mut _opaque_pthread_t;
pub type pthread_t = __darwin_pthread_t;
pub type pid_t = __darwin_pid_t;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct iovec {
    pub iov_base: *mut ::libc::c_void,
    pub iov_len: usize,
}
pub type mdbx_pid_t = pid_t;
pub type mdbx_tid_t = pthread_t;
pub type mdbx_mode_t = mode_t;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_version_info {
    pub major: u8,
    pub minor: u8,
    pub release: u16,
    pub revision: u32,
    pub git: MDBX_version_info__bindgen_ty_1,
    pub sourcery: *const ::libc::c_char,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_version_info__bindgen_ty_1 {
    pub datetime: *const ::libc::c_char,
    pub tree: *const ::libc::c_char,
    pub commit: *const ::libc::c_char,
    pub describe: *const ::libc::c_char,
}
extern "C" {
    pub static mdbx_version: MDBX_version_info;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_build_info {
    pub datetime: *const ::libc::c_char,
    pub target: *const ::libc::c_char,
    pub options: *const ::libc::c_char,
    pub compiler: *const ::libc::c_char,
    pub flags: *const ::libc::c_char,
}
extern "C" {
    pub static mdbx_build: MDBX_build_info;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_env {
    _unused: [u8; 0],
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_txn {
    _unused: [u8; 0],
}
pub type MDBX_dbi = u32;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_cursor {
    _unused: [u8; 0],
}
pub type MDBX_val = iovec;
pub const MDBX_MAX_DBI: MDBX_constants = 32765;
pub const MDBX_MAXDATASIZE: MDBX_constants = 2147418112;
pub const MDBX_MIN_PAGESIZE: MDBX_constants = 256;
pub const MDBX_MAX_PAGESIZE: MDBX_constants = 65536;
pub type MDBX_constants = ::libc::c_uint;
pub const MDBX_LOG_FATAL: MDBX_log_level_t = 0;
pub const MDBX_LOG_ERROR: MDBX_log_level_t = 1;
pub const MDBX_LOG_WARN: MDBX_log_level_t = 2;
pub const MDBX_LOG_NOTICE: MDBX_log_level_t = 3;
pub const MDBX_LOG_VERBOSE: MDBX_log_level_t = 4;
pub const MDBX_LOG_DEBUG: MDBX_log_level_t = 5;
pub const MDBX_LOG_TRACE: MDBX_log_level_t = 6;
pub const MDBX_LOG_EXTRA: MDBX_log_level_t = 7;
pub const MDBX_LOG_DONTCHANGE: MDBX_log_level_t = -1;
pub type MDBX_log_level_t = ::libc::c_int;
pub const MDBX_DBG_ASSERT: MDBX_debug_flags_t = 1;
pub const MDBX_DBG_AUDIT: MDBX_debug_flags_t = 2;
pub const MDBX_DBG_JITTER: MDBX_debug_flags_t = 4;
pub const MDBX_DBG_DUMP: MDBX_debug_flags_t = 8;
pub const MDBX_DBG_LEGACY_MULTIOPEN: MDBX_debug_flags_t = 16;
pub const MDBX_DBG_LEGACY_OVERLAP: MDBX_debug_flags_t = 32;
pub const MDBX_DBG_DONTCHANGE: MDBX_debug_flags_t = -1;
pub type MDBX_debug_flags_t = ::libc::c_int;
pub type MDBX_debug_func = ::std::option::Option<
    unsafe extern "C" fn(
        loglevel: MDBX_log_level_t,
        function: *const ::libc::c_char,
        line: ::libc::c_int,
        msg: *const ::libc::c_char,
        args: *mut __va_list_tag,
    ),
>;
extern "C" {
    pub fn mdbx_setup_debug(
        log_level: MDBX_log_level_t,
        debug_flags: MDBX_debug_flags_t,
        logger: MDBX_debug_func,
    ) -> ::libc::c_int;
}
pub type MDBX_assert_func = ::std::option::Option<
    unsafe extern "C" fn(
        env: *const MDBX_env,
        msg: *const ::libc::c_char,
        function: *const ::libc::c_char,
        line: ::libc::c_uint,
    ),
>;
extern "C" {
    pub fn mdbx_env_set_assert(env: *mut MDBX_env, func: MDBX_assert_func) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_dump_val(
        key: *const MDBX_val,
        buf: *mut ::libc::c_char,
        bufsize: usize,
    ) -> *const ::libc::c_char;
}
extern "C" {
    pub fn mdbx_panic(fmt: *const ::libc::c_char, ...);
}
pub const MDBX_ENV_DEFAULTS: MDBX_env_flags_t = 0;
pub const MDBX_NOSUBDIR: MDBX_env_flags_t = 16384;
pub const MDBX_RDONLY: MDBX_env_flags_t = 131072;
pub const MDBX_EXCLUSIVE: MDBX_env_flags_t = 4194304;
pub const MDBX_ACCEDE: MDBX_env_flags_t = 1073741824;
pub const MDBX_WRITEMAP: MDBX_env_flags_t = 524288;
pub const MDBX_NOTLS: MDBX_env_flags_t = 2097152;
pub const MDBX_NORDAHEAD: MDBX_env_flags_t = 8388608;
pub const MDBX_NOMEMINIT: MDBX_env_flags_t = 16777216;
pub const MDBX_COALESCE: MDBX_env_flags_t = 33554432;
pub const MDBX_LIFORECLAIM: MDBX_env_flags_t = 67108864;
pub const MDBX_PAGEPERTURB: MDBX_env_flags_t = 134217728;
pub const MDBX_SYNC_DURABLE: MDBX_env_flags_t = 0;
pub const MDBX_NOMETASYNC: MDBX_env_flags_t = 262144;
pub const MDBX_SAFE_NOSYNC: MDBX_env_flags_t = 65536;
pub const MDBX_MAPASYNC: MDBX_env_flags_t = 65536;
pub const MDBX_UTTERLY_NOSYNC: MDBX_env_flags_t = 1114112;
pub type MDBX_env_flags_t = ::libc::c_uint;
pub const MDBX_TXN_READWRITE: MDBX_txn_flags_t = 0;
pub const MDBX_TXN_RDONLY: MDBX_txn_flags_t = 131072;
pub const MDBX_TXN_RDONLY_PREPARE: MDBX_txn_flags_t = 16908288;
pub const MDBX_TXN_TRY: MDBX_txn_flags_t = 268435456;
pub const MDBX_TXN_NOMETASYNC: MDBX_txn_flags_t = 262144;
pub const MDBX_TXN_NOSYNC: MDBX_txn_flags_t = 65536;
pub type MDBX_txn_flags_t = ::libc::c_uint;
pub const MDBX_DB_DEFAULTS: MDBX_db_flags_t = 0;
pub const MDBX_REVERSEKEY: MDBX_db_flags_t = 2;
pub const MDBX_DUPSORT: MDBX_db_flags_t = 4;
pub const MDBX_INTEGERKEY: MDBX_db_flags_t = 8;
pub const MDBX_DUPFIXED: MDBX_db_flags_t = 16;
pub const MDBX_INTEGERDUP: MDBX_db_flags_t = 32;
pub const MDBX_REVERSEDUP: MDBX_db_flags_t = 64;
pub const MDBX_CREATE: MDBX_db_flags_t = 262144;
pub const MDBX_DB_ACCEDE: MDBX_db_flags_t = 1073741824;
pub type MDBX_db_flags_t = ::libc::c_uint;
pub const MDBX_UPSERT: MDBX_put_flags_t = 0;
pub const MDBX_NOOVERWRITE: MDBX_put_flags_t = 16;
pub const MDBX_NODUPDATA: MDBX_put_flags_t = 32;
pub const MDBX_CURRENT: MDBX_put_flags_t = 64;
pub const MDBX_ALLDUPS: MDBX_put_flags_t = 128;
pub const MDBX_RESERVE: MDBX_put_flags_t = 65536;
pub const MDBX_APPEND: MDBX_put_flags_t = 131072;
pub const MDBX_APPENDDUP: MDBX_put_flags_t = 262144;
pub const MDBX_MULTIPLE: MDBX_put_flags_t = 524288;
pub type MDBX_put_flags_t = ::libc::c_uint;
pub const MDBX_CP_DEFAULTS: MDBX_copy_flags_t = 0;
pub const MDBX_CP_COMPACT: MDBX_copy_flags_t = 1;
pub const MDBX_CP_FORCE_DYNAMIC_SIZE: MDBX_copy_flags_t = 2;
pub type MDBX_copy_flags_t = ::libc::c_uint;
#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum MDBX_cursor_op {
    MDBX_FIRST = 0,
    MDBX_FIRST_DUP = 1,
    MDBX_GET_BOTH = 2,
    MDBX_GET_BOTH_RANGE = 3,
    MDBX_GET_CURRENT = 4,
    MDBX_GET_MULTIPLE = 5,
    MDBX_LAST = 6,
    MDBX_LAST_DUP = 7,
    MDBX_NEXT = 8,
    MDBX_NEXT_DUP = 9,
    MDBX_NEXT_MULTIPLE = 10,
    MDBX_NEXT_NODUP = 11,
    MDBX_PREV = 12,
    MDBX_PREV_DUP = 13,
    MDBX_PREV_NODUP = 14,
    MDBX_SET = 15,
    MDBX_SET_KEY = 16,
    MDBX_SET_RANGE = 17,
    MDBX_PREV_MULTIPLE = 18,
    MDBX_SET_LOWERBOUND = 19,
}
pub const MDBX_SUCCESS: MDBX_error_t = 0;
pub const MDBX_RESULT_FALSE: MDBX_error_t = 0;
pub const MDBX_RESULT_TRUE: MDBX_error_t = -1;
pub const MDBX_KEYEXIST: MDBX_error_t = -30799;
pub const MDBX_FIRST_LMDB_ERRCODE: MDBX_error_t = -30799;
pub const MDBX_NOTFOUND: MDBX_error_t = -30798;
pub const MDBX_PAGE_NOTFOUND: MDBX_error_t = -30797;
pub const MDBX_CORRUPTED: MDBX_error_t = -30796;
pub const MDBX_PANIC: MDBX_error_t = -30795;
pub const MDBX_VERSION_MISMATCH: MDBX_error_t = -30794;
pub const MDBX_INVALID: MDBX_error_t = -30793;
pub const MDBX_MAP_FULL: MDBX_error_t = -30792;
pub const MDBX_DBS_FULL: MDBX_error_t = -30791;
pub const MDBX_READERS_FULL: MDBX_error_t = -30790;
pub const MDBX_TXN_FULL: MDBX_error_t = -30788;
pub const MDBX_CURSOR_FULL: MDBX_error_t = -30787;
pub const MDBX_PAGE_FULL: MDBX_error_t = -30786;
pub const MDBX_UNABLE_EXTEND_MAPSIZE: MDBX_error_t = -30785;
pub const MDBX_INCOMPATIBLE: MDBX_error_t = -30784;
pub const MDBX_BAD_RSLOT: MDBX_error_t = -30783;
pub const MDBX_BAD_TXN: MDBX_error_t = -30782;
pub const MDBX_BAD_VALSIZE: MDBX_error_t = -30781;
pub const MDBX_BAD_DBI: MDBX_error_t = -30780;
pub const MDBX_PROBLEM: MDBX_error_t = -30779;
pub const MDBX_LAST_LMDB_ERRCODE: MDBX_error_t = -30779;
pub const MDBX_BUSY: MDBX_error_t = -30778;
pub const MDBX_FIRST_ADDED_ERRCODE: MDBX_error_t = -30778;
pub const MDBX_EMULTIVAL: MDBX_error_t = -30421;
pub const MDBX_EBADSIGN: MDBX_error_t = -30420;
pub const MDBX_WANNA_RECOVERY: MDBX_error_t = -30419;
pub const MDBX_EKEYMISMATCH: MDBX_error_t = -30418;
pub const MDBX_TOO_LARGE: MDBX_error_t = -30417;
pub const MDBX_THREAD_MISMATCH: MDBX_error_t = -30416;
pub const MDBX_TXN_OVERLAPPING: MDBX_error_t = -30415;
pub const MDBX_LAST_ADDED_ERRCODE: MDBX_error_t = -30415;
pub const MDBX_ENODATA: MDBX_error_t = 96;
pub const MDBX_EINVAL: MDBX_error_t = 22;
pub const MDBX_EACCESS: MDBX_error_t = 13;
pub const MDBX_ENOMEM: MDBX_error_t = 12;
pub const MDBX_EROFS: MDBX_error_t = 30;
pub const MDBX_ENOSYS: MDBX_error_t = 78;
pub const MDBX_EIO: MDBX_error_t = 5;
pub const MDBX_EPERM: MDBX_error_t = 1;
pub const MDBX_EINTR: MDBX_error_t = 4;
pub const MDBX_ENOFILE: MDBX_error_t = 2;
pub const MDBX_EREMOTE: MDBX_error_t = 15;
pub type MDBX_error_t = ::libc::c_int;
extern "C" {
    pub fn mdbx_strerror(errnum: ::libc::c_int) -> *const ::libc::c_char;
}
extern "C" {
    pub fn mdbx_strerror_r(
        errnum: ::libc::c_int,
        buf: *mut ::libc::c_char,
        buflen: usize,
    ) -> *const ::libc::c_char;
}
extern "C" {
    pub fn mdbx_liberr2str(errnum: ::libc::c_int) -> *const ::libc::c_char;
}
extern "C" {
    pub fn mdbx_env_create(penv: *mut *mut MDBX_env) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_open(
        env: *mut MDBX_env,
        pathname: *const ::libc::c_char,
        flags: MDBX_env_flags_t,
        mode: mdbx_mode_t,
    ) -> ::libc::c_int;
}
pub const MDBX_ENV_JUST_DELETE: MDBX_env_delete_mode_t = 0;
pub const MDBX_ENV_ENSURE_UNUSED: MDBX_env_delete_mode_t = 1;
pub const MDBX_ENV_WAIT_FOR_UNUSED: MDBX_env_delete_mode_t = 2;
pub type MDBX_env_delete_mode_t = ::libc::c_uint;
extern "C" {
    pub fn mdbx_env_delete(
        pathname: *const ::libc::c_char,
        mode: MDBX_env_delete_mode_t,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_copy(
        env: *mut MDBX_env,
        dest: *const ::libc::c_char,
        flags: MDBX_copy_flags_t,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_copy2fd(
        env: *mut MDBX_env,
        fd: mdbx_filehandle_t,
        flags: MDBX_copy_flags_t,
    ) -> ::libc::c_int;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_stat {
    pub ms_psize: u32,
    pub ms_depth: u32,
    pub ms_branch_pages: u64,
    pub ms_leaf_pages: u64,
    pub ms_overflow_pages: u64,
    pub ms_entries: u64,
    pub ms_mod_txnid: u64,
}
extern "C" {
    pub fn mdbx_env_stat_ex(
        env: *const MDBX_env,
        txn: *const MDBX_txn,
        stat: *mut MDBX_stat,
        bytes: usize,
    ) -> ::libc::c_int;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_envinfo {
    pub mi_geo: MDBX_envinfo__bindgen_ty_1,
    pub mi_mapsize: u64,
    pub mi_last_pgno: u64,
    pub mi_recent_txnid: u64,
    pub mi_latter_reader_txnid: u64,
    pub mi_self_latter_reader_txnid: u64,
    pub mi_meta0_txnid: u64,
    pub mi_meta0_sign: u64,
    pub mi_meta1_txnid: u64,
    pub mi_meta1_sign: u64,
    pub mi_meta2_txnid: u64,
    pub mi_meta2_sign: u64,
    pub mi_maxreaders: u32,
    pub mi_numreaders: u32,
    pub mi_dxb_pagesize: u32,
    pub mi_sys_pagesize: u32,
    pub mi_bootid: MDBX_envinfo__bindgen_ty_2,
    pub mi_unsync_volume: u64,
    pub mi_autosync_threshold: u64,
    pub mi_since_sync_seconds16dot16: u32,
    pub mi_autosync_period_seconds16dot16: u32,
    pub mi_since_reader_check_seconds16dot16: u32,
    pub mi_mode: u32,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_envinfo__bindgen_ty_1 {
    pub lower: u64,
    pub upper: u64,
    pub current: u64,
    pub shrink: u64,
    pub grow: u64,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_envinfo__bindgen_ty_2 {
    pub current: MDBX_envinfo__bindgen_ty_2__bindgen_ty_1,
    pub meta0: MDBX_envinfo__bindgen_ty_2__bindgen_ty_1,
    pub meta1: MDBX_envinfo__bindgen_ty_2__bindgen_ty_1,
    pub meta2: MDBX_envinfo__bindgen_ty_2__bindgen_ty_1,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_envinfo__bindgen_ty_2__bindgen_ty_1 {
    pub x: u64,
    pub y: u64,
}
extern "C" {
    pub fn mdbx_env_info_ex(
        env: *const MDBX_env,
        txn: *const MDBX_txn,
        info: *mut MDBX_envinfo,
        bytes: usize,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_sync_ex(env: *mut MDBX_env, force: bool, nonblock: bool) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_set_syncbytes(env: *mut MDBX_env, threshold: usize) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_set_syncperiod(
        env: *mut MDBX_env,
        seconds_16dot16: ::libc::c_uint,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_close_ex(env: *mut MDBX_env, dont_sync: bool) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_set_flags(
        env: *mut MDBX_env,
        flags: MDBX_env_flags_t,
        onoff: bool,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_get_flags(env: *const MDBX_env, flags: *mut ::libc::c_uint) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_get_path(
        env: *const MDBX_env,
        dest: *mut *const ::libc::c_char,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_get_fd(env: *const MDBX_env, fd: *mut mdbx_filehandle_t) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_set_geometry(
        env: *mut MDBX_env,
        size_lower: isize,
        size_now: isize,
        size_upper: isize,
        growth_step: isize,
        shrink_threshold: isize,
        pagesize: isize,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_is_readahead_reasonable(volume: usize, redundancy: isize) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_limits_dbsize_min(pagesize: isize) -> isize;
}
extern "C" {
    pub fn mdbx_limits_dbsize_max(pagesize: isize) -> isize;
}
extern "C" {
    pub fn mdbx_limits_keysize_max(pagesize: isize, flags: MDBX_db_flags_t) -> isize;
}
extern "C" {
    pub fn mdbx_limits_valsize_max(pagesize: isize, flags: MDBX_db_flags_t) -> isize;
}
extern "C" {
    pub fn mdbx_limits_txnsize_max(pagesize: isize) -> isize;
}
extern "C" {
    pub fn mdbx_env_set_maxreaders(env: *mut MDBX_env, readers: ::libc::c_uint) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_get_maxreaders(
        env: *const MDBX_env,
        readers: *mut ::libc::c_uint,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_set_maxdbs(env: *mut MDBX_env, dbs: MDBX_dbi) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_get_maxdbs(env: *mut MDBX_env, dbs: *mut MDBX_dbi) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_get_maxkeysize_ex(
        env: *const MDBX_env,
        flags: MDBX_db_flags_t,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_get_maxvalsize_ex(
        env: *const MDBX_env,
        flags: MDBX_db_flags_t,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_get_maxkeysize(env: *const MDBX_env) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_set_userctx(env: *mut MDBX_env, ctx: *mut ::libc::c_void) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_get_userctx(env: *const MDBX_env) -> *mut ::libc::c_void;
}
extern "C" {
    pub fn mdbx_txn_begin_ex(
        env: *mut MDBX_env,
        parent: *mut MDBX_txn,
        flags: MDBX_txn_flags_t,
        txn: *mut *mut MDBX_txn,
        context: *mut ::libc::c_void,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_txn_set_userctx(txn: *mut MDBX_txn, ctx: *mut ::libc::c_void) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_txn_get_userctx(txn: *const MDBX_txn) -> *mut ::libc::c_void;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_txn_info {
    pub txn_id: u64,
    pub txn_reader_lag: u64,
    pub txn_space_used: u64,
    pub txn_space_limit_soft: u64,
    pub txn_space_limit_hard: u64,
    pub txn_space_retired: u64,
    pub txn_space_leftover: u64,
    pub txn_space_dirty: u64,
}
extern "C" {
    pub fn mdbx_txn_info(
        txn: *const MDBX_txn,
        info: *mut MDBX_txn_info,
        scan_rlt: bool,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_txn_env(txn: *const MDBX_txn) -> *mut MDBX_env;
}
extern "C" {
    pub fn mdbx_txn_flags(txn: *const MDBX_txn) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_txn_id(txn: *const MDBX_txn) -> u64;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_commit_latency {
    pub preparation: u32,
    pub gc: u32,
    pub audit: u32,
    pub write: u32,
    pub sync: u32,
    pub ending: u32,
    pub whole: u32,
}
extern "C" {
    pub fn mdbx_txn_commit_ex(
        txn: *mut MDBX_txn,
        latency: *mut MDBX_commit_latency,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_txn_abort(txn: *mut MDBX_txn) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_txn_break(txn: *mut MDBX_txn) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_txn_reset(txn: *mut MDBX_txn) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_txn_renew(txn: *mut MDBX_txn) -> ::libc::c_int;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MDBX_canary {
    pub x: u64,
    pub y: u64,
    pub z: u64,
    pub v: u64,
}
extern "C" {
    pub fn mdbx_canary_put(txn: *mut MDBX_txn, canary: *const MDBX_canary) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_canary_get(txn: *const MDBX_txn, canary: *mut MDBX_canary) -> ::libc::c_int;
}
pub type MDBX_cmp_func = ::std::option::Option<
    unsafe extern "C" fn(a: *const MDBX_val, b: *const MDBX_val) -> ::libc::c_int,
>;
extern "C" {
    pub fn mdbx_dbi_open(
        txn: *mut MDBX_txn,
        name: *const ::libc::c_char,
        flags: MDBX_db_flags_t,
        dbi: *mut MDBX_dbi,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_dbi_open_ex(
        txn: *mut MDBX_txn,
        name: *const ::libc::c_char,
        flags: MDBX_db_flags_t,
        dbi: *mut MDBX_dbi,
        keycmp: MDBX_cmp_func,
        datacmp: MDBX_cmp_func,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_key_from_jsonInteger(json_integer: i64) -> u64;
}
extern "C" {
    pub fn mdbx_key_from_double(ieee754_64bit: f64) -> u64;
}
extern "C" {
    pub fn mdbx_key_from_ptrdouble(ieee754_64bit: *const f64) -> u64;
}
extern "C" {
    pub fn mdbx_key_from_float(ieee754_32bit: f32) -> u32;
}
extern "C" {
    pub fn mdbx_key_from_ptrfloat(ieee754_32bit: *const f32) -> u32;
}
extern "C" {
    pub fn mdbx_jsonInteger_from_key(arg1: MDBX_val) -> i64;
}
extern "C" {
    pub fn mdbx_double_from_key(arg1: MDBX_val) -> f64;
}
extern "C" {
    pub fn mdbx_float_from_key(arg1: MDBX_val) -> f32;
}
extern "C" {
    pub fn mdbx_int32_from_key(arg1: MDBX_val) -> i32;
}
extern "C" {
    pub fn mdbx_int64_from_key(arg1: MDBX_val) -> i64;
}
extern "C" {
    pub fn mdbx_dbi_stat(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        stat: *mut MDBX_stat,
        bytes: usize,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_dbi_dupsort_depthmask(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        mask: *mut u32,
    ) -> ::libc::c_int;
}
pub const MDBX_DBI_DIRTY: MDBX_dbi_state_t = 1;
pub const MDBX_DBI_STALE: MDBX_dbi_state_t = 2;
pub const MDBX_DBI_FRESH: MDBX_dbi_state_t = 4;
pub const MDBX_DBI_CREAT: MDBX_dbi_state_t = 8;
pub type MDBX_dbi_state_t = ::libc::c_uint;
extern "C" {
    pub fn mdbx_dbi_flags_ex(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        flags: *mut ::libc::c_uint,
        state: *mut ::libc::c_uint,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_dbi_close(env: *mut MDBX_env, dbi: MDBX_dbi) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_drop(txn: *mut MDBX_txn, dbi: MDBX_dbi, del: bool) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_get(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        key: *const MDBX_val,
        data: *mut MDBX_val,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_get_ex(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        key: *mut MDBX_val,
        data: *mut MDBX_val,
        values_count: *mut usize,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_get_equal_or_great(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        key: *mut MDBX_val,
        data: *mut MDBX_val,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_put(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        key: *const MDBX_val,
        data: *mut MDBX_val,
        flags: MDBX_put_flags_t,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_replace(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        key: *const MDBX_val,
        new_data: *mut MDBX_val,
        old_data: *mut MDBX_val,
        flags: MDBX_put_flags_t,
    ) -> ::libc::c_int;
}
pub type MDBX_preserve_func = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut ::libc::c_void,
        target: *mut MDBX_val,
        src: *const ::libc::c_void,
        bytes: usize,
    ) -> ::libc::c_int,
>;
extern "C" {
    pub fn mdbx_replace_ex(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        key: *const MDBX_val,
        new_data: *mut MDBX_val,
        old_data: *mut MDBX_val,
        flags: MDBX_put_flags_t,
        preserver: MDBX_preserve_func,
        preserver_context: *mut ::libc::c_void,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_del(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        key: *const MDBX_val,
        data: *const MDBX_val,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_create(context: *mut ::libc::c_void) -> *mut MDBX_cursor;
}
extern "C" {
    pub fn mdbx_cursor_set_userctx(
        cursor: *mut MDBX_cursor,
        ctx: *mut ::libc::c_void,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_get_userctx(cursor: *const MDBX_cursor) -> *mut ::libc::c_void;
}
extern "C" {
    pub fn mdbx_cursor_bind(
        txn: *mut MDBX_txn,
        cursor: *mut MDBX_cursor,
        dbi: MDBX_dbi,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_open(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        cursor: *mut *mut MDBX_cursor,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_close(cursor: *mut MDBX_cursor);
}
extern "C" {
    pub fn mdbx_cursor_renew(txn: *mut MDBX_txn, cursor: *mut MDBX_cursor) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_txn(cursor: *const MDBX_cursor) -> *mut MDBX_txn;
}
extern "C" {
    pub fn mdbx_cursor_dbi(cursor: *const MDBX_cursor) -> MDBX_dbi;
}
extern "C" {
    pub fn mdbx_cursor_copy(src: *const MDBX_cursor, dest: *mut MDBX_cursor) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_get(
        cursor: *mut MDBX_cursor,
        key: *mut MDBX_val,
        data: *mut MDBX_val,
        op: MDBX_cursor_op,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_put(
        cursor: *mut MDBX_cursor,
        key: *const MDBX_val,
        data: *mut MDBX_val,
        flags: MDBX_put_flags_t,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_del(cursor: *mut MDBX_cursor, flags: MDBX_put_flags_t) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_count(cursor: *const MDBX_cursor, pcount: *mut usize) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_eof(cursor: *const MDBX_cursor) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_on_first(cursor: *const MDBX_cursor) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cursor_on_last(cursor: *const MDBX_cursor) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_estimate_distance(
        first: *const MDBX_cursor,
        last: *const MDBX_cursor,
        distance_items: *mut isize,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_estimate_move(
        cursor: *const MDBX_cursor,
        key: *mut MDBX_val,
        data: *mut MDBX_val,
        move_op: MDBX_cursor_op,
        distance_items: *mut isize,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_estimate_range(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        begin_key: *mut MDBX_val,
        begin_data: *mut MDBX_val,
        end_key: *mut MDBX_val,
        end_data: *mut MDBX_val,
        distance_items: *mut isize,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_is_dirty(txn: *const MDBX_txn, ptr: *const ::libc::c_void) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_dbi_sequence(
        txn: *mut MDBX_txn,
        dbi: MDBX_dbi,
        result: *mut u64,
        increment: u64,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_cmp(
        txn: *const MDBX_txn,
        dbi: MDBX_dbi,
        a: *const MDBX_val,
        b: *const MDBX_val,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_get_keycmp(flags: MDBX_db_flags_t) -> MDBX_cmp_func;
}
extern "C" {
    pub fn mdbx_dcmp(
        txn: *const MDBX_txn,
        dbi: MDBX_dbi,
        a: *const MDBX_val,
        b: *const MDBX_val,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_get_datacmp(flags: MDBX_db_flags_t) -> MDBX_cmp_func;
}
pub type MDBX_reader_list_func = ::std::option::Option<
    unsafe extern "C" fn(
        ctx: *mut ::libc::c_void,
        num: ::libc::c_int,
        slot: ::libc::c_int,
        pid: mdbx_pid_t,
        thread: mdbx_tid_t,
        txnid: u64,
        lag: u64,
        bytes_used: usize,
        bytes_retained: usize,
    ) -> ::libc::c_int,
>;
extern "C" {
    pub fn mdbx_reader_list(
        env: *const MDBX_env,
        func: MDBX_reader_list_func,
        ctx: *mut ::libc::c_void,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_reader_check(env: *mut MDBX_env, dead: *mut ::libc::c_int) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_txn_straggler(txn: *const MDBX_txn, percent: *mut ::libc::c_int) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_thread_register(env: *const MDBX_env) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_thread_unregister(env: *const MDBX_env) -> ::libc::c_int;
}
pub type MDBX_hsr_func = ::std::option::Option<
    unsafe extern "C" fn(
        env: *const MDBX_env,
        txn: *const MDBX_txn,
        pid: mdbx_pid_t,
        tid: mdbx_tid_t,
        laggard: u64,
        gap: ::libc::c_uint,
        space: usize,
        retry: ::libc::c_int,
    ) -> ::libc::c_int,
>;
extern "C" {
    pub fn mdbx_env_set_hsr(env: *mut MDBX_env, hsr_callback: MDBX_hsr_func) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_get_hsr(env: *const MDBX_env) -> MDBX_hsr_func;
}
#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum MDBX_page_type_t {
    MDBX_page_broken = 0,
    MDBX_page_meta = 1,
    MDBX_page_large = 2,
    MDBX_page_branch = 3,
    MDBX_page_leaf = 4,
    MDBX_page_dupfixed_leaf = 5,
    MDBX_subpage_leaf = 6,
    MDBX_subpage_dupfixed_leaf = 7,
    MDBX_subpage_broken = 8,
}
pub type MDBX_pgvisitor_func = ::std::option::Option<
    unsafe extern "C" fn(
        pgno: u64,
        number: ::libc::c_uint,
        ctx: *mut ::libc::c_void,
        deep: ::libc::c_int,
        dbi: *const ::libc::c_char,
        page_size: usize,
        type_: MDBX_page_type_t,
        err: MDBX_error_t,
        nentries: usize,
        payload_bytes: usize,
        header_bytes: usize,
        unused_bytes: usize,
    ) -> ::libc::c_int,
>;
extern "C" {
    pub fn mdbx_env_pgwalk(
        txn: *mut MDBX_txn,
        visitor: MDBX_pgvisitor_func,
        ctx: *mut ::libc::c_void,
        dont_check_keys_ordering: bool,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_open_for_recovery(
        env: *mut MDBX_env,
        pathname: *const ::libc::c_char,
        target_meta: ::libc::c_uint,
        writeable: bool,
    ) -> ::libc::c_int;
}
extern "C" {
    pub fn mdbx_env_turn_for_recovery(
        env: *mut MDBX_env,
        target_meta: ::libc::c_uint,
    ) -> ::libc::c_int;
}
pub type __builtin_va_list = [__va_list_tag; 1usize];
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct __va_list_tag {
    pub gp_offset: ::libc::c_uint,
    pub fp_offset: ::libc::c_uint,
    pub overflow_arg_area: *mut ::libc::c_void,
    pub reg_save_area: *mut ::libc::c_void,
}
