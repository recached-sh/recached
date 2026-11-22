samples: 10879, user cycles/op 3034

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  28.0%    849.7 | `decode (all)` |
|  18.8%    570.8 | `decode: general Value path (Args::general)` |
|  14.1%    428.3 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|   0.0%      0.0 | `borrowed execute (try_execute_borrowed)` |
|  55.3%   1676.8 | `ordered write entry (execute_ordered_write)` |
|  51.5%   1562.8 | `store execute (all)` |
|   5.1%    155.9 | `store: Command clone` |
|   6.2%    187.4 | `store: key hash (sip)` |
|   5.5%    167.0 | `store: entry_state` |
|   3.1%     92.6 | `store: extract_key` |
|   5.9%    179.6 | `clock reads` |
|   7.2%    218.6 | `metrics record` |
|   0.3%     10.6 | `reply serialize (Value)` |
|   1.1%     32.9 | `Value drop` |
|   0.5%     16.5 | `Command drop` |
|   7.8%    237.9 | `allocator (all)` |
|   0.9%     26.5 | `select overflow arm (mpsc recv)` |
|   1.5%     46.0 | `BufWriter flush (all)` |
|   2.6%     80.3 | `AofGatedWriter (all)` |
|   0.2%      7.3 | `DeferredAof pop` |
|   0.6%     17.3 | `socket read (poll_read_priv)` |
|   0.4%     13.4 | `socket write (poll_write_priv)` |
|   2.6%     80.3 | `split lock (ReadHalf/WriteHalf)` |
|   0.4%     13.1 | `tokio park/driver` |
|  99.1%   3007.0 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  36.5% |    1107 |
| connection (decode, reply, socket) | allocator |   5.6% |     171 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   3.7% |     112 |
| connection (decode, reply, socket) | tokio mpsc |   0.5% |      16 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.2% |       7 |
| **connection (decode, reply, socket)** | **all** | ** 46.5%** | **   1412** |
| store execute | code |  34.8% |    1056 |
| store execute | libc (memcpy, syscall wrappers) |   7.2% |     217 |
| store execute | allocator |   2.2% |      66 |
| **store execute** | **all** | ** 44.1%** | **   1339** |
| ordered write path (rest) | code |   3.8% |     114 |
| **ordered write path (rest)** | **all** | **  3.8%** | **    114** |
| tokio scheduler | tokio wake/schedule |   1.9% |      57 |
| tokio scheduler | code |   0.5% |      16 |
| tokio scheduler | tokio io/split |   0.3% |      10 |
| tokio scheduler | allocator |   0.0% |       1 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.0% |       1 |
| **tokio scheduler** | **all** | **  2.8%** | **     84** |
| AOF append/lead/gate | tokio io/split |   1.7% |      50 |
| AOF append/lead/gate | code |   0.5% |      16 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   0.5% |      16 |
| **AOF append/lead/gate** | **all** | **  2.7%** | **     82** |
| tokio io driver | code |   0.1% |       2 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.0% |       1 |
| **tokio io driver** | **all** | **  0.1%** | **      3** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|   7.5%    228.4 | `<core_engine::request::RequestDecoder>::decode_any` |
|   6.7%    204.4 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   5.5%    165.4 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.1%    155.9 | `<core_engine::cmd::Command>::from_value` |
|   4.2%    127.4 | `_rjem_malloc` |
|   3.5%    107.6 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   3.2%     98.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   3.2%     96.8 | `recached_server::clients::record_metrics` |
|   3.0%     92.3 | `[vdso]:+a8c` |
|   3.0%     92.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   2.9%     89.0 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   2.8%     86.2 | `<core_engine::store::KeyValueStore>::execute_inner` |
|   2.6%     79.8 | `_rjem_sdallocx` |
|   1.8%     54.7 | `<core_engine::request::Args>::general` |
|   1.8%     53.3 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   1.7%     51.9 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.6%     47.4 | `<core_engine::cmd::Command as core::clone::Clone>::clone` |
|   1.3%     39.9 | `<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<&str>, smallvec::IntoI…` |
|   1.2%     37.4 | `core::ptr::drop_glue::<core_engine::store::SlotRef>` |
|   1.2%     36.0 | `core_engine::resp::format_i64` |
|   1.2%     35.1 | `<core_engine::store::KeyValueStore>::entry_state` |
|   1.1%     34.6 | `[unknown]` |
|   1.1%     34.0 | `<core::str::lossy::Utf8Chunks as core::iter::traits::iterator::Iterator>::next` |
|   1.1%     32.9 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   1.1%     32.9 | `core_engine::store::lock_stripe` |
|   1.1%     32.1 | `<alloc::vec::Vec<alloc::vec::Vec<u8>> as alloc::vec::spec_from_iter_nested::SpecFromIterNested<alloc::vec::Ve…` |
|   0.9%     28.4 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.8%     25.1 | `core_engine::request::build_ref` |
|   0.8%     24.8 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.8%     24.3 | `<alloc::string::String>::from_utf8_lossy` |
|   0.8%     23.7 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.8%     23.4 | `recached_server::clients::execute_and_record_ref` |
|   0.7%     22.3 | `<str>::to_uppercase` |
|   0.7%     20.1 | `core_engine::request::build` |
|   0.6%     19.2 | `core_engine::store::now_ms` |
|   0.6%     18.4 | `core::str::converts::from_utf8` |
|   0.6%     18.1 | `libc.so.6:+1c791d` |
|   0.6%     17.3 | `core_engine::cmd::extract_string` |
|   0.6%     17.0 | `<alloc::vec::Vec<alloc::vec::Vec<u8>> as core::clone::Clone>::clone` |
|   0.5%     15.6 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   0.5%     15.6 | `libc.so.6:+1c7919` |
|   0.5%     15.3 | `core_engine::resp::write_header` |
|   0.5%     15.3 | `core_engine::store::write_slot::<alloc::string::String>` |
|   0.5%     14.8 | `recached_server::propagation::is_write_command` |
|   0.4%     13.4 | `<std::sys::pal::unix::time::Timespec>::now` |
|   0.4%     13.1 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   0.4%     12.3 | `core_engine::store::mutation_scope` |
|   0.4%     12.0 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   0.4%     12.0 | `core::ptr::drop_glue::<alloc::vec::Vec<alloc::vec::Vec<u8>>>` |
|   0.4%     11.4 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   0.4%     11.2 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.4%     11.2 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   0.4%     10.9 | `core_engine::cmd::extract_key` |
|   0.3%     10.6 | `<core_engine::resp::Value>::serialize_into` |
|   0.3%     10.3 | `core_engine::store::entry_size` |
|   0.3%     10.0 | `[vdso]:+ac2` |
|   0.3%     10.0 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   0.3%      9.5 | `libc.so.6:+1c7da5` |
|   0.3%      9.5 | `libc.so.6:+1c7de1` |
|   0.3%      9.2 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%   3033.8 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  99.1%   3007.0 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  98.6%   2991.1 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  98.2%   2977.8 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  97.6%   2960.5 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  55.1%   1670.4 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  51.5%   1562.8 | `recached_server::clients::execute_and_record_ref` |
|  41.0%   1242.9 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  28.0%    849.7 | `<core_engine::request::RequestDecoder>::decode_any` |
|  18.8%    570.8 | `<core_engine::request::Args>::general` |
|  14.5%    440.6 | `<core_engine::store::KeyValueStore>::execute_inner` |
|  14.1%    428.3 | `<core_engine::cmd::Command>::from_value` |
|   7.5%    226.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   7.2%    218.6 | `recached_server::clients::record_metrics` |
|   5.9%    179.6 | `<std::sys::pal::unix::time::Timespec>::now` |
|   5.5%    167.0 | `<core_engine::store::KeyValueStore>::entry_state` |
|   5.5%    165.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   5.1%    155.9 | `<core_engine::cmd::Command as core::clone::Clone>::clone` |
|   5.1%    154.2 | `libc.so.6:+139b5b` |
|   4.8%    146.7 | `core_engine::store::write_slot::<alloc::string::String>` |
|   4.3%    131.3 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   4.2%    127.4 | `_rjem_malloc` |
|   3.1%     92.6 | `core_engine::cmd::extract_key` |
|   3.0%     92.3 | `[vdso]:+a8c` |
|   3.0%     91.7 | `core_engine::store::now_ms` |
|   3.0%     90.4 | `<alloc::vec::Vec<alloc::vec::Vec<u8>> as core::clone::Clone>::clone` |
|   2.9%     89.0 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   2.9%     89.0 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   2.7%     81.7 | `core_engine::cmd::extract_string` |
|   2.6%     79.8 | `_rjem_sdallocx` |
|   2.0%     60.8 | `core_engine::resp::write_header` |
|   1.9%     58.3 | `<alloc::string::String>::from_utf8_lossy` |
|   1.5%     46.0 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.5%     46.0 | `<alloc::collections::vec_deque::VecDeque<core_engine::store::Blob>>::grow` |
|   1.5%     45.5 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   1.5%     44.6 | `core::ptr::drop_glue::<core_engine::store::SlotRef>` |
|   1.4%     43.5 | `<alloc::vec::Vec<alloc::vec::Vec<u8>> as alloc::vec::spec_from_iter_nested::SpecFromIterNested<alloc::vec::Ve…` |
|   1.4%     42.7 | `_rjem_je_malloc_default` |
|   1.3%     39.9 | `<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<&str>, smallvec::IntoI…` |
|   1.2%     37.1 | `<alloc::raw_vec::RawVecInner>::finish_grow` |
|   1.2%     36.8 | `_rjem_je_tcache_alloc_small_hard` |
|   1.2%     36.8 | `_rjem_je_arena_cache_bin_fill_small` |
|   1.2%     36.8 | `_rjem_je_large_ralloc` |
|   1.2%     36.8 | `do_rallocx` |
|   1.2%     36.8 | `<alloc::raw_vec::RawVec<(alloc::string::String, bool)>>::grow_one` |
|   1.2%     36.0 | `core_engine::resp::format_i64` |
|   1.2%     35.1 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.1%     34.6 | `[unknown]` |
|   1.1%     34.3 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   1.1%     34.0 | `<core::str::lossy::Utf8Chunks as core::iter::traits::iterator::Iterator>::next` |
|   1.1%     34.0 | `core_engine::request::build` |
|   1.1%     32.9 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   1.1%     32.9 | `core_engine::store::lock_stripe` |
|   1.1%     32.6 | `<str>::to_uppercase` |
|   1.0%     31.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.9%     28.4 | `<std::sys::pal::unix::time::Timespec>::sub_timespec` |
|   0.9%     26.5 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.8%     25.1 | `core_engine::request::build_ref` |
|   0.8%     24.8 | `arena_slab_alloc` |
|   0.7%     22.6 | `core::ptr::drop_glue::<alloc::vec::Vec<alloc::vec::Vec<u8>>>` |
