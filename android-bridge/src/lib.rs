use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jstring};
use jni::JNIEnv;
use reader_core::epub::EpubBook;
use reader_core::i18n::{I18n, Language};

macro_rules! dbg_log {
    ($($arg:tt)*) => {
        reader_core::sharing::share_dbg_log(&format!($($arg)*));
    };
}

/// Safely create a JNI string and return its raw pointer.
/// Returns null on failure instead of panicking.
macro_rules! jni_string_or_null {
    ($env:expr, $val:expr) => {
        match $env.new_string($val) {
            Ok(s) => s.into_raw(),
            Err(e) => {
                dbg_log!("JNI new_string failed: {}", e);
                std::ptr::null_mut()
            }
        }
    };
}

#[allow(unused_macros)]
/// Safely lock a mutex, returning null_mut() jstring on failure.
macro_rules! lock_or_null {
    ($mutex:expr) => {
        match $mutex.lock() {
            Ok(guard) => guard,
            Err(e) => {
                dbg_log!("Mutex lock poisoned: {}", e);
                return std::ptr::null_mut();
            }
        }
    };
}

#[allow(unused_macros)]
/// Safely lock a mutex, returning () on failure (for void JNI functions).
macro_rules! lock_or_return {
    ($mutex:expr) => {
        match $mutex.lock() {
            Ok(guard) => guard,
            Err(e) => {
                dbg_log!("Mutex lock poisoned: {}", e);
                return;
            }
        }
    };
}
use reader_core::library::Library;
use reader_core::sharing::protocol::Message;
use reader_core::sharing::{
    auto_sync_session, connect_to_peer, handle_client, resolve_broadcast_addr, start_broadcast,
    start_listener, start_server, DiscoveredPeer, DiscoveryAnnouncement, PeerStore,
};
use reader_core::{base64_encode, now_secs};

use lru::LruCache;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// Global cache for parsed EPUB books to prevent re-parsing the ZIP file and HTML
// every time Java requests a chapter or cover. Cache size is 3 to prevent OOM.
static BOOK_CACHE: Lazy<Mutex<LruCache<String, Arc<EpubBook>>>> =
    Lazy::new(|| Mutex::new(LruCache::new(NonZeroUsize::new(3).unwrap())));

fn get_book(path: &str) -> std::result::Result<Arc<EpubBook>, String> {
    let mut cache = BOOK_CACHE
        .lock()
        .map_err(|e| format!("BOOK_CACHE lock poisoned: {}", e))?;
    if let Some(book) = cache.get(path) {
        return Ok(book.clone());
    }

    // We need to parse it if not in cache
    match EpubBook::open(path) {
        Ok(book) => {
            let arc_book = Arc::new(book);
            cache.put(path.to_string(), arc_book.clone());
            Ok(arc_book)
        }
        Err(err) => Err(err),
    }
}

fn to_android_last_opened(ts: u64) -> u64 {
    // Rust core stores seconds; historical Android data may be millis.
    // Normalize to millis for Android UI sorting compatibility.
    if ts < 10_000_000_000 {
        ts.saturating_mul(1000)
    } else {
        ts
    }
}

fn book_entry_to_android_json(
    data_dir: &str,
    entry: &reader_core::library::BookEntry,
) -> serde_json::Value {
    let config_path = if entry.id.is_empty() {
        None
    } else {
        Some(
            std::path::PathBuf::from(data_dir)
                .join("books")
                .join(format!("{}.json", entry.id))
                .to_string_lossy()
                .to_string(),
        )
    };

    serde_json::json!({
        "id": entry.id.clone(),
        "title": entry.title.clone(),
        "uri": entry.path.clone(),
        "config_path": config_path,
        "lastChapter": entry.last_chapter,
        "last_chapter_title": entry.last_chapter_title.clone(),
        "lastOpened": to_android_last_opened(entry.last_opened),
    })
}

/// Open an EPUB file and return its metadata as JSON.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_openBook(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
) -> jstring {
    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };

    let book = match get_book(&path) {
        Ok(b) => b,
        Err(_) => return std::ptr::null_mut(),
    };

    let json = serde_json::json!({
        "title": book.title,
        "chapterCount": book.chapters.len(),
        "toc": book.toc.iter().map(|t| serde_json::json!({
            "title": t.title,
            "chapterIndex": t.chapter_index,
        })).collect::<Vec<_>>(),
        "hasCover": book.cover_data.is_some(),
    });

    jni_string_or_null!(env, json.to_string())
}

/// Get chapter content as JSON array of content blocks.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_getChapter(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
    chapter_index: jni::sys::jint,
) -> jstring {
    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };

    let book = match get_book(&path) {
        Ok(b) => b,
        Err(_) => return std::ptr::null_mut(),
    };

    let idx = chapter_index as usize;
    if idx >= book.chapters.len() {
        return std::ptr::null_mut();
    }

    let chapter = &book.chapters[idx];
    let blocks: Vec<serde_json::Value> = chapter
        .blocks
        .iter()
        .map(|block| match block {
            reader_core::epub::ContentBlock::Paragraph { spans } => {
                serde_json::json!({
                    "type": "paragraph",
                    "spans": spans.iter().map(|s| serde_json::json!({
                        "text": s.text,
                        "style": format!("{:?}", s.style),
                        "linkUrl": s.link_url,
                    })).collect::<Vec<_>>(),
                })
            }
            reader_core::epub::ContentBlock::Heading { level, spans } => {
                serde_json::json!({
                    "type": "heading",
                    "level": level,
                    "spans": spans.iter().map(|s| serde_json::json!({
                        "text": s.text,
                        "style": format!("{:?}", s.style),
                        "linkUrl": s.link_url,
                    })).collect::<Vec<_>>(),
                })
            }
            reader_core::epub::ContentBlock::Separator => {
                serde_json::json!({ "type": "separator" })
            }
            reader_core::epub::ContentBlock::BlankLine => {
                serde_json::json!({ "type": "blankLine" })
            }
            reader_core::epub::ContentBlock::Image { data, alt } => {
                serde_json::json!({
                    "type": "image",
                    "data": base64_encode(data),
                    "alt": alt,
                })
            }
        })
        .collect();

    let json = serde_json::json!({
        "title": chapter.title,
        "sourceHref": chapter.source_href,
        "blocks": blocks,
    });

    jni_string_or_null!(env, json.to_string())
}

/// Get the cover image bytes (Base64 encoded) for a book.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_getCover(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
) -> jstring {
    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };

    let book = match get_book(&path) {
        Ok(b) => b,
        Err(_) => return std::ptr::null_mut(),
    };

    match &book.cover_data {
        Some(data) => {
            let encoded = base64_encode(data);
            jni_string_or_null!(env, encoded)
        }
        None => std::ptr::null_mut(),
    }
}

/// Load the library index from the data directory.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_loadLibrary(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
) -> jstring {
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };

    let library = Library::load_from(&data_dir);
    let entries: Vec<serde_json::Value> = library
        .books
        .iter()
        .map(|e| book_entry_to_android_json(&data_dir, e))
        .collect();

    let output =
        env.new_string(serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string()));
    match output {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Add or update a book in the library.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_addOrUpdateBook(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    title: JString,
    path: JString,
    chapter: jni::sys::jint,
    chapter_title: JString,
) -> jstring {
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let title: String = match env.get_string(&title) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let chapter_title: String = match env.get_string(&chapter_title) {
        Ok(s) => s.into(),
        Err(_) => String::new(),
    };
    let chapter_title_opt = if chapter_title.trim().is_empty() {
        None
    } else {
        Some(chapter_title)
    };

    let mut library = Library::load_from(&data_dir);
    let entry = library.add_or_update(&data_dir, title, path, chapter as usize, chapter_title_opt);
    let json_str = book_entry_to_android_json(&data_dir, &entry).to_string();
    jni_string_or_null!(env, json_str)
}

/// Update the current chapter for a book.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_updateChapter(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    path: JString,
    chapter: jni::sys::jint,
    chapter_title: JString,
) {
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return,
    };
    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return,
    };
    let chapter_title: String = match env.get_string(&chapter_title) {
        Ok(s) => s.into(),
        Err(_) => String::new(),
    };
    let chapter_title_opt = if chapter_title.trim().is_empty() {
        None
    } else {
        Some(chapter_title)
    };

    let mut library = Library::load_from(&data_dir);
    library.update_chapter(
        &data_dir,
        &path,
        chapter as usize,
        chapter_title_opt.clone(),
    );

    // Keep sharing progress in PeerStore in sync with local reading progress,
    // so subsequent auto_sync_session uploads latest chapter/chapter_title.
    if let Ok(book_hash) = EpubBook::file_hash(&path) {
        let title = library
            .books
            .iter()
            .find(|b| b.path == path)
            .map(|b| b.title.clone())
            .unwrap_or_else(|| {
                std::path::Path::new(&path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });

        let store = get_peer_store(&data_dir);
        let mut s = store.lock().unwrap_or_else(|e| {
            dbg_log!("PeerStore lock poisoned, recovering: {}", e);
            e.into_inner()
        });
        let ts = now_secs();
        if let Some(local) = s.progress.iter_mut().find(|p| p.book_hash == book_hash) {
            local.title = title;
            local.chapter = chapter as usize;
            local.chapter_title = chapter_title_opt;
            local.timestamp = ts;
        } else {
            s.progress.push(reader_core::sharing::ProgressEntry {
                book_hash,
                title,
                chapter: chapter as usize,
                chapter_title: chapter_title_opt,
                timestamp: ts,
            });
        }
        s.save(&data_dir);
    }
}

/// Remove a book from the library by index.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_removeBook(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    index: jni::sys::jint,
) {
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return,
    };

    let mut library = Library::load_from(&data_dir);
    library.remove(&data_dir, index as usize);
}

/// Remove a book from the library by absolute path.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_removeBookByPath(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    path: JString,
) -> jboolean {
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };

    let mut library = Library::load_from(&data_dir);
    let before = library.books.len();
    library.remove_by_path(&data_dir, &path);
    let after = library.books.len();
    if after < before {
        1
    } else {
        0
    }
}

/// Return the available language list as JSON.
/// JSON format: [{"code":"zh_cn","label":"中文"}, ...]
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_getAvailableLanguages(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let langs: Vec<serde_json::Value> = Language::all()
        .iter()
        .map(|l| {
            serde_json::json!({
                "code": l.code(),
                "label": l.label(),
            })
        })
        .collect();
    let json = serde_json::json!(langs);
    jni_string_or_null!(env, json.to_string())
}

/// Return the full translation map for the given language code as JSON.
/// Pass "auto" to get the default (ZhCN) translations.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_getTranslations(
    mut env: JNIEnv,
    _class: JClass,
    lang_code: JString,
) -> jstring {
    let code: String = match env.get_string(&lang_code) {
        Ok(s) => s.into(),
        Err(_) => "zh_cn".to_string(),
    };
    let language = Language::from_code(&code);
    let i18n = I18n::new(language);

    let translations = i18n.get_all_translations();
    let json = serde_json::to_string(translations).unwrap_or_else(|_| "{}".to_string());
    jni_string_or_null!(env, json)
}

// ── Sharing global state ──

struct SharingServer {
    stop_flag: Arc<AtomicBool>,
    #[allow(dead_code)]
    addr: String,
}

static SHARING_SERVER: Lazy<Mutex<Option<SharingServer>>> = Lazy::new(|| Mutex::new(None));

/// Global PeerStore shared across all JNI calls to prevent race conditions
/// where multiple PeerStore::load() calls generate different device_ids.
type GlobalPeerStore = Option<(String, Arc<Mutex<PeerStore>>)>;
static GLOBAL_PEER_STORE: Lazy<Mutex<GlobalPeerStore>> = Lazy::new(|| Mutex::new(None));

/// Get or create the global PeerStore for the given data_dir.
fn get_peer_store(data_dir: &str) -> Arc<Mutex<PeerStore>> {
    let mut guard = GLOBAL_PEER_STORE.lock().unwrap_or_else(|e| {
        dbg_log!("GLOBAL_PEER_STORE lock poisoned, recovering: {}", e);
        e.into_inner()
    });
    if let Some((ref dir, ref store)) = *guard {
        if dir == data_dir {
            return store.clone();
        }
    }
    let store = Arc::new(Mutex::new(PeerStore::load(data_dir)));
    *guard = Some((data_dir.to_string(), store.clone()));
    store
}

/// Collect all existing book file paths from library.json for sharing fallback.
/// This is important when some books are not physically under books_dir yet
/// (e.g. during migration / mixed historical data).
fn collect_library_book_paths(data_dir: &str) -> Vec<String> {
    let library = Library::load_from(data_dir);
    let mut seen = HashSet::new();
    let mut paths = Vec::new();

    for entry in library.books {
        if entry.path.trim().is_empty() {
            continue;
        }
        if !std::path::Path::new(&entry.path).exists() {
            continue;
        }
        if seen.insert(entry.path.clone()) {
            paths.push(entry.path);
        }
    }

    paths
}

// ── Discovery global state ──

struct DiscoveryListenerState {
    stop_flag: Arc<AtomicBool>,
    peers: Arc<Mutex<Vec<DiscoveredPeer>>>,
}

static DISCOVERY_LISTENER: Lazy<Mutex<Option<DiscoveryListenerState>>> =
    Lazy::new(|| Mutex::new(None));

/// Generate a random 4-digit PIN.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_generatePin(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;
    let mut hasher = DefaultHasher::new();
    SystemTime::now().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    let pin = format!("{:04}", hasher.finish() % 10000);
    jni_string_or_null!(env, &pin)
}

/// Start the sharing server. Returns JSON: {"ok": true, "addr": "ip:port"} or {"ok": false, "error": "..."}
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_startSharingServer(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    books_dir_j: JString,
    pin_j: JString,
) -> jstring {
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let books_dir: String = match env.get_string(&books_dir_j) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let pin: String = match env.get_string(&pin_j) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    dbg_log!(
        "JNI startSharingServer: data_dir={} books_dir={} pin='{}'",
        data_dir,
        books_dir,
        pin
    );
    let initial_extra_paths = collect_library_book_paths(&data_dir);
    dbg_log!(
        "JNI startSharingServer: initial library fallback paths={} ",
        initial_extra_paths.len()
    );

    // Stop any existing server
    {
        let mut guard = SHARING_SERVER.lock().unwrap_or_else(|e| {
            dbg_log!("SHARING_SERVER lock poisoned, recovering: {}", e);
            e.into_inner()
        });
        if let Some(server) = guard.take() {
            server.stop_flag.store(true, Ordering::SeqCst);
        }
    }

    let store = get_peer_store(&data_dir);
    let (device_id, device_name) = {
        let s = store.lock().unwrap_or_else(|e| {
            dbg_log!("PeerStore lock poisoned, recovering: {}", e);
            e.into_inner()
        });
        (s.device_id.clone(), s.device_name.clone())
    };

    match start_server("0.0.0.0:0", &data_dir, &books_dir, &pin, store.clone()) {
        Ok((listener, addr)) => {
            let resolved_addr = resolve_broadcast_addr(&addr);
            dbg_log!(
                "JNI startSharingServer: bound to {} resolved to {}",
                addr,
                resolved_addr
            );
            let stop_flag = Arc::new(AtomicBool::new(false));
            let sf = stop_flag.clone();

            // Broadcast our presence so other devices can discover us
            start_broadcast(
                DiscoveryAnnouncement {
                    device_id,
                    device_name,
                    addr: resolved_addr.clone(),
                },
                stop_flag.clone(),
            );

            {
                let mut guard = SHARING_SERVER.lock().unwrap_or_else(|e| {
                    dbg_log!("SHARING_SERVER lock poisoned, recovering: {}", e);
                    e.into_inner()
                });
                *guard = Some(SharingServer {
                    stop_flag: stop_flag.clone(),
                    addr: resolved_addr.clone(),
                });
            }

            listener.set_nonblocking(true).ok();

            std::thread::spawn(move || loop {
                if sf.load(Ordering::SeqCst) {
                    break;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(std::time::Duration::from_secs(60)))
                            .ok();
                        stream.set_nonblocking(false).ok();
                        let dd = data_dir.clone();
                        let bd = books_dir.clone();
                        let p = pin.clone();
                        let s = store.clone();
                        std::thread::spawn(move || {
                            let extra_paths = collect_library_book_paths(&dd);
                            dbg_log!(
                                "JNI startSharingServer: handle_client fallback paths={}",
                                extra_paths.len()
                            );
                            let _ = handle_client(&mut stream, &dd, &bd, &p, s, &extra_paths);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                    Err(_) => break,
                }
            });

            let json = serde_json::json!({"ok": true, "addr": resolved_addr});
            jni_string_or_null!(env, json.to_string())
        }
        Err(e) => {
            let json = serde_json::json!({"ok": false, "error": e});
            jni_string_or_null!(env, json.to_string())
        }
    }
}

/// Stop the sharing server.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_stopSharingServer(
    _env: JNIEnv,
    _class: JClass,
) {
    let mut guard = SHARING_SERVER.lock().unwrap_or_else(|e| {
        dbg_log!("SHARING_SERVER lock poisoned, recovering: {}", e);
        e.into_inner()
    });
    if let Some(server) = guard.take() {
        server.stop_flag.store(true, Ordering::SeqCst);
    }
}

/// Connect to a peer and sync. Returns JSON: {"ok": true, "books": [...]} or {"error": "..."}.
/// Parameters: addr, pin, device_id, data_dir, books_dir
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_connectAndListBooks(
    mut env: JNIEnv,
    _class: JClass,
    addr: JString,
    pin_str: JString,
    device_id_str: JString,
    data_dir: JString,
    books_dir: JString,
) -> jstring {
    let addr: String = match env.get_string(&addr) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let pin_str: String = match env.get_string(&pin_str) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let device_id_str: String = match env.get_string(&device_id_str) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let books_dir: String = match env.get_string(&books_dir) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let extra_book_paths = collect_library_book_paths(&data_dir);

    let store_arc = get_peer_store(&data_dir);
    let mut store = store_arc
        .lock()
        .unwrap_or_else(|e| {
            dbg_log!("PeerStore lock poisoned, recovering: {}", e);
            e.into_inner()
        })
        .clone();
    let pin_opt = if pin_str.is_empty() {
        None
    } else {
        Some(pin_str.as_str())
    };
    let dev_id_opt = if device_id_str.is_empty() {
        None
    } else {
        Some(device_id_str.as_str())
    };
    dbg_log!(
        "JNI connectAndListBooks: addr={} pin='{}' device_id={:?} fallback_paths={}",
        addr,
        pin_opt.unwrap_or("(none)"),
        dev_id_opt,
        extra_book_paths.len()
    );
    match connect_to_peer(&addr, &mut store, &data_dir, dev_id_opt, pin_opt) {
        Ok((mut stream, aes_key, mut send_ctr, mut recv_ctr)) => {
            dbg_log!("JNI connectAndListBooks: connect_to_peer OK, starting sync...");
            *store_arc.lock().unwrap_or_else(|e| {
                dbg_log!("PeerStore lock poisoned, recovering: {}", e);
                e.into_inner()
            }) = store.clone();
            match auto_sync_session(
                &mut stream,
                &aes_key,
                &mut send_ctr,
                &mut recv_ctr,
                &mut store,
                &data_dir,
                &books_dir,
                &extra_book_paths,
            ) {
                Ok((_changed_progress, fetched_books)) => {
                    dbg_log!(
                        "JNI connectAndListBooks: sync OK, {} books fetched",
                        fetched_books.len()
                    );
                    // Keep in-memory global store in sync with the latest merged progress/pairing state.
                    *store_arc.lock().unwrap_or_else(|e| {
                        dbg_log!("PeerStore lock poisoned, recovering: {}", e);
                        e.into_inner()
                    }) = store.clone();
                    let json =
                        serde_json::to_string(&fetched_books).unwrap_or_else(|_| "[]".into());
                    jni_string_or_null!(env, &json)
                }
                Err(e) => {
                    dbg_log!("JNI connectAndListBooks: sync error: {}", e);
                    // Even on error, store may have partial updates; keep global cache consistent.
                    *store_arc.lock().unwrap_or_else(|e| {
                        dbg_log!("PeerStore lock poisoned, recovering: {}", e);
                        e.into_inner()
                    }) = store.clone();
                    let json = serde_json::json!({"error": e, "phase": "sync"});
                    jni_string_or_null!(env, json.to_string())
                }
            }
        }
        Err(e) => {
            dbg_log!("JNI connectAndListBooks: connect error: {}", e);
            let json = serde_json::json!({"error": e, "phase": "connect"});
            jni_string_or_null!(env, json.to_string())
        }
    }
}

/// Request and download a book from a peer by hash.
/// Note: with the new encrypted protocol, this does a full connect+auth cycle.
/// Returns JSON: {"ok": true, "path": "..."} or {"ok": false, "error": "..."}
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_requestBookFromPeer(
    mut env: JNIEnv,
    _class: JClass,
    addr: JString,
    pin_str: JString,
    device_id_str: JString,
    data_dir: JString,
    books_dir: JString,
    hash: JString,
) -> jstring {
    let addr: String = match env.get_string(&addr) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let pin_str: String = match env.get_string(&pin_str) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let device_id_str: String = match env.get_string(&device_id_str) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let _books_dir: String = match env.get_string(&books_dir) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let hash: String = match env.get_string(&hash) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };

    let store_arc = get_peer_store(&data_dir);
    let mut store = store_arc
        .lock()
        .unwrap_or_else(|e| {
            dbg_log!("PeerStore lock poisoned, recovering: {}", e);
            e.into_inner()
        })
        .clone();
    let pin_opt = if pin_str.is_empty() {
        None
    } else {
        Some(pin_str.as_str())
    };
    let dev_id_opt = if device_id_str.is_empty() {
        None
    } else {
        Some(device_id_str.as_str())
    };

    match connect_to_peer(&addr, &mut store, &data_dir, dev_id_opt, pin_opt) {
        Ok((mut stream, aes_key, mut send_ctr, mut recv_ctr)) => {
            *store_arc.lock().unwrap_or_else(|e| {
                dbg_log!("PeerStore lock poisoned, recovering: {}", e);
                e.into_inner()
            }) = store.clone();
            use reader_core::sharing::crypto;
            if crypto::write_encrypted_message(
                &mut stream,
                &Message::RequestBook { hash: hash.clone() },
                &aes_key,
                &mut send_ctr,
            )
            .is_ok()
            {
                match crypto::read_encrypted_message(&mut stream, &aes_key, &mut recv_ctr) {
                    Ok(Message::BookData { title, .. }) => {
                        if let Ok(data) =
                            crypto::read_encrypted_raw(&mut stream, &aes_key, &mut recv_ctr)
                        {
                            let mut library = Library::load_from(&data_dir);
                            let entry = library.add_or_update_from_bytes(
                                &data_dir,
                                title.clone(),
                                &data,
                                0,
                                None,
                            );
                            let json = serde_json::json!({
                                "ok": true,
                                "path": entry.path,
                                "title": entry.title,
                                "id": entry.id,
                            });
                            return jni_string_or_null!(env, json.to_string());
                        }
                    }
                    Ok(Message::BookNotFound { .. }) => {
                        let json = serde_json::json!({"ok": false, "error": "Book not found"});
                        return jni_string_or_null!(env, json.to_string());
                    }
                    _ => {}
                }
            }
            let json = serde_json::json!({"ok": false, "error": "Download failed"});
            jni_string_or_null!(env, json.to_string())
        }
        Err(e) => {
            let json = serde_json::json!({"ok": false, "error": e});
            jni_string_or_null!(env, json.to_string())
        }
    }
}

/// Sync reading progress with a peer. Returns JSON: {"ok": true} or {"ok": false, "error": "..."}
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_syncProgressWithPeer(
    mut env: JNIEnv,
    _class: JClass,
    addr: JString,
    pin_str: JString,
    device_id_str: JString,
    data_dir: JString,
    books_dir: JString,
) -> jstring {
    let addr: String = match env.get_string(&addr) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let pin_str: String = match env.get_string(&pin_str) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let device_id_str: String = match env.get_string(&device_id_str) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let books_dir: String = match env.get_string(&books_dir) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let extra_book_paths = collect_library_book_paths(&data_dir);

    let store_arc = get_peer_store(&data_dir);
    let mut store = store_arc
        .lock()
        .unwrap_or_else(|e| {
            dbg_log!("PeerStore lock poisoned, recovering: {}", e);
            e.into_inner()
        })
        .clone();
    let pin_opt = if pin_str.is_empty() {
        None
    } else {
        Some(pin_str.as_str())
    };
    let dev_id_opt = if device_id_str.is_empty() {
        None
    } else {
        Some(device_id_str.as_str())
    };

    match connect_to_peer(&addr, &mut store, &data_dir, dev_id_opt, pin_opt) {
        Ok((mut stream, aes_key, mut send_ctr, mut recv_ctr)) => {
            *store_arc.lock().unwrap_or_else(|e| {
                dbg_log!("PeerStore lock poisoned, recovering: {}", e);
                e.into_inner()
            }) = store.clone();
            match auto_sync_session(
                &mut stream,
                &aes_key,
                &mut send_ctr,
                &mut recv_ctr,
                &mut store,
                &data_dir,
                &books_dir,
                &extra_book_paths,
            ) {
                Ok(_) => {
                    // Keep in-memory global store in sync with latest merged progress.
                    *store_arc.lock().unwrap_or_else(|e| {
                        dbg_log!("PeerStore lock poisoned, recovering: {}", e);
                        e.into_inner()
                    }) = store.clone();
                    let json = serde_json::json!({"ok": true});
                    jni_string_or_null!(env, json.to_string())
                }
                Err(e) => {
                    // Even on error, store may have partial updates; keep global cache consistent.
                    *store_arc.lock().unwrap_or_else(|e| {
                        dbg_log!("PeerStore lock poisoned, recovering: {}", e);
                        e.into_inner()
                    }) = store.clone();
                    let json = serde_json::json!({"ok": false, "error": e, "phase": "sync"});
                    jni_string_or_null!(env, json.to_string())
                }
            }
        }
        Err(e) => {
            *store_arc.lock().unwrap_or_else(|e| {
                dbg_log!("PeerStore lock poisoned, recovering: {}", e);
                e.into_inner()
            }) = store.clone();
            let json = serde_json::json!({"ok": false, "error": e, "phase": "connect"});
            jni_string_or_null!(env, json.to_string())
        }
    }
}

/// Get list of paired devices as JSON array.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_getPairedDevices(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
) -> jstring {
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };

    let store = get_peer_store(&data_dir);
    let s = store.lock().unwrap_or_else(|e| {
        dbg_log!("PeerStore lock poisoned, recovering: {}", e);
        e.into_inner()
    });
    let json = serde_json::to_string(&s.paired).unwrap_or_else(|_| "[]".into());
    jni_string_or_null!(env, &json)
}

/// Start the UDP discovery listener so this device can find active sharing servers.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_removePairedDevice(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    device_id: JString,
) -> jboolean {
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    let device_id: String = match env.get_string(&device_id) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    let store = get_peer_store(&data_dir);
    let mut s = store.lock().unwrap_or_else(|e| {
        dbg_log!("PeerStore lock poisoned, recovering: {}", e);
        e.into_inner()
    });
    let removed = s.remove_paired(&device_id);
    if removed {
        s.save(&data_dir);
    }
    if removed {
        1
    } else {
        0
    }
}

/// Start the UDP discovery listener so this device can find active sharing servers.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_startDiscoveryListener(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
) {
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return,
    };
    let store = get_peer_store(&data_dir);
    let own_id = store
        .lock()
        .unwrap_or_else(|e| {
            dbg_log!("PeerStore lock poisoned, recovering: {}", e);
            e.into_inner()
        })
        .device_id
        .clone();
    dbg_log!("JNI startDiscoveryListener: own_id={}", own_id);

    // Stop any existing listener first
    {
        let mut guard = DISCOVERY_LISTENER.lock().unwrap_or_else(|e| {
            dbg_log!("DISCOVERY_LISTENER lock poisoned, recovering: {}", e);
            e.into_inner()
        });
        if let Some(prev) = guard.take() {
            prev.stop_flag.store(true, Ordering::SeqCst);
        }
    }

    let stop_flag = Arc::new(AtomicBool::new(false));
    let peers = start_listener(&own_id, stop_flag.clone());

    let mut guard = DISCOVERY_LISTENER.lock().unwrap_or_else(|e| {
        dbg_log!("DISCOVERY_LISTENER lock poisoned, recovering: {}", e);
        e.into_inner()
    });
    *guard = Some(DiscoveryListenerState { stop_flag, peers });
}

/// Stop the UDP discovery listener.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_stopDiscoveryListener(
    _env: JNIEnv,
    _class: JClass,
) {
    let mut guard = DISCOVERY_LISTENER.lock().unwrap_or_else(|e| {
        dbg_log!("DISCOVERY_LISTENER lock poisoned, recovering: {}", e);
        e.into_inner()
    });
    if let Some(state) = guard.take() {
        state.stop_flag.store(true, Ordering::SeqCst);
    }
}

/// Return the current list of discovered peers as a JSON array.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_getDiscoveredPeers(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let guard = DISCOVERY_LISTENER.lock().unwrap_or_else(|e| {
        dbg_log!("DISCOVERY_LISTENER lock poisoned, recovering: {}", e);
        e.into_inner()
    });
    let json = if let Some(state) = &*guard {
        let peers = match state.peers.lock() {
            Ok(p) => p,
            Err(e) => {
                dbg_log!("peers lock poisoned: {}", e);
                return jni_string_or_null!(env, "[]");
            }
        };
        dbg_log!("JNI getDiscoveredPeers: {} peers found", peers.len());
        for p in peers.iter() {
            dbg_log!(
                "  peer: id={} name='{}' addr={} last_seen={}",
                p.device_id,
                p.device_name,
                p.addr,
                p.last_seen
            );
        }
        serde_json::to_string(&*peers).unwrap_or_else(|_| "[]".into())
    } else {
        "[]".into()
    };
    jni_string_or_null!(env, &json)
}

/// Return the synced progress entries from PeerStore as JSON array.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_getSyncedProgress(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
) -> jstring {
    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let store = get_peer_store(&data_dir);
    let progress = store
        .lock()
        .unwrap_or_else(|e| {
            dbg_log!("PeerStore lock poisoned, recovering: {}", e);
            e.into_inner()
        })
        .progress
        .clone();
    let json = serde_json::to_string(&progress).unwrap_or_else(|_| "[]".into());
    jni_string_or_null!(env, &json)
}

/// Compute SHA-256 hash of a file (same as EpubBook::file_hash).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_fileHash(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
) -> jstring {
    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    match reader_core::epub::EpubBook::file_hash(&path) {
        Ok(hash) => {
            jni_string_or_null!(env, &hash)
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Read epub metadata as JSON without full parsing.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_zhongbai233_epub_reader_RustBridge_readEpubMetadata(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
) -> jstring {
    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    match reader_core::epub::EpubBook::read_metadata(&path) {
        Some(meta) => match serde_json::to_string(&meta) {
            Ok(json) => {
                jni_string_or_null!(env, &json)
            }
            Err(_) => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}
