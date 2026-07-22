use std::{
    fs,
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use bincode::Options;
use serde::{Deserialize, Serialize};
use steel::{
    compiler::{
        modules::steel_home,
        program::{Executable, RawProgramWithSymbols, SerializableRawProgramWithSymbols},
    },
    steel_vm::engine::Engine,
    steelerr, SteelVal,
};

use crate::commands::Context;

const STEEL_COMPAT_REV: &str = "785149c";
const ARTIFACT_FILE: &str = "steel-init.bin";
const MANIFEST_FILE: &str = "steel-init.manifest";
const ARTIFACT_SIZE_LIMIT: u64 = 512 * 1024 * 1024;

pub struct CachePaths {
    artifact: PathBuf,
    manifest: PathBuf,
}

impl CachePaths {
    fn standard() -> Self {
        let dir = helix_loader::cache_dir();
        CachePaths {
            artifact: dir.join(ARTIFACT_FILE),
            manifest: dir.join(MANIFEST_FILE),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CacheKey {
    hx_build: String,
    steel_compat: String,
    steel_home: String,
}

impl CacheKey {
    fn current() -> Self {
        CacheKey {
            hx_build: helix_loader::VERSION_AND_GIT_HASH.to_string(),
            steel_compat: STEEL_COMPAT_REV.to_string(),
            steel_home: steel_home().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SourceStamp {
    path: PathBuf,
    mtime_secs: u64,
    mtime_nanos: u32,
    size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct InitCacheManifest {
    hx_build: String,
    steel_compat: String,
    steel_home: String,
    hx_binary: SourceStamp,
    init_script: SourceStamp,
    modules: Vec<SourceStamp>,
    artifact_len: u64,
    artifact_checksum: u64,
}

pub fn run_init_script(
    engine: &mut Engine,
    cx: &mut Context,
    bind_to: &'static str,
    contents: String,
    path: PathBuf,
) -> steel::rvals::Result<SteelVal> {
    let paths = CachePaths::standard();
    let key = CacheKey::current();

    let load_started = std::time::Instant::now();

    if let Some(cached) = load_cached_program(&paths, &key, &path) {
        log::info!("Steel init cache load: {:?}", load_started.elapsed());
        let prepare_started = std::time::Instant::now();
        match prepare_cached_executable(engine, cached) {
            Ok(executable) => {
                log::info!(
                    "Steel init cache hit: {:?} (prepare {:?})",
                    paths.artifact,
                    prepare_started.elapsed()
                );
                let run_started = std::time::Instant::now();
                match run_cached_executable(engine, cx, bind_to, &executable) {
                    Ok(value) => {
                        log::info!("Steel init cache run: {:?}", run_started.elapsed());
                        return Ok(value);
                    }
                    Err(e) => {
                        log::warn!(
                            "Steel init cache program failed ({}), discarding and recompiling from source",
                            e
                        );
                        discard(&paths);
                    }
                }
            }
            Err(reason) => {
                log::warn!("Steel init cache unusable ({}), discarding", reason);
                discard(&paths);
            }
        }
    }

    log::info!("Steel init cache miss, compiling {:?} from source", path);
    compile_run_and_persist(engine, cx, bind_to, &contents, &path, &paths, &key)
}

fn run_cached_executable(
    engine: &mut Engine,
    cx: &mut Context,
    bind_to: &'static str,
    executable: &Executable,
) -> steel::rvals::Result<SteelVal> {
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
        engine.run_thunk_with_reference::<Context, Context>(cx, |engine, ctx_value| {
            engine.update_value(bind_to, ctx_value);
            let res = engine.run_executable(executable);
            engine.update_value(bind_to, SteelVal::Void);
            res.map(|values| values.into_iter().next().unwrap_or(SteelVal::Void))
        })
    }));

    match outcome {
        Ok(res) => res,
        Err(_) => steelerr!(Generic => "cached init program panicked; recompile on next launch"),
    }
}

fn compile_run_and_persist(
    engine: &mut Engine,
    cx: &mut Context,
    bind_to: &'static str,
    contents: &str,
    path: &Path,
    paths: &CachePaths,
    key: &CacheKey,
) -> steel::rvals::Result<SteelVal> {
    engine.run_thunk_with_reference::<Context, Context>(cx, |engine, ctx_value| {
        engine.update_value(bind_to, ctx_value);
        let res = engine
            .emit_raw_program(contents.to_owned(), path.to_path_buf())
            .and_then(|program| {
                let outcome = engine.run_raw_program(program.clone());
                if outcome.is_ok() {
                    persist(engine, program, path, paths, key);
                }
                outcome
            });
        engine.update_value(bind_to, SteelVal::Void);
        res.map(|values| values.into_iter().next().unwrap_or(SteelVal::Void))
    })
}

fn prepare_cached_executable(
    engine: &mut Engine,
    cached: RawProgramWithSymbols,
) -> Result<Executable, String> {
    engine
        .raw_program_to_executable(cached)
        .map_err(|e| format!("linking cached program: {}", e))
}

fn load_cached_program(
    paths: &CachePaths,
    key: &CacheKey,
    init_path: &Path,
) -> Option<RawProgramWithSymbols> {
    let manifest_bytes = fs::read(&paths.manifest).ok()?;
    let manifest: InitCacheManifest = match serde_json::from_slice(&manifest_bytes) {
        Ok(manifest) => manifest,
        Err(e) => return discard_corrupt(paths, format!("manifest unreadable: {}", e)),
    };
    let exe_path = std::env::current_exe().ok()?;
    if !manifest_is_current(&manifest, key, init_path, &exe_path, &observe_stamp) {
        return None;
    }
    let artifact = match fs::read(&paths.artifact) {
        Ok(artifact) => artifact,
        Err(e) => return discard_corrupt(paths, format!("artifact unreadable: {}", e)),
    };
    if artifact.len() as u64 != manifest.artifact_len
        || fnv1a(&artifact) != manifest.artifact_checksum
    {
        return discard_corrupt(paths, "artifact does not match its manifest".to_string());
    }
    match decode_image(&artifact) {
        Some(cached) => Some(cached),
        None => discard_corrupt(paths, "artifact failed to decode".to_string()),
    }
}

fn discard_corrupt(paths: &CachePaths, reason: String) -> Option<RawProgramWithSymbols> {
    log::warn!("Steel init cache corrupt ({}), discarding", reason);
    discard(paths);
    None
}

fn decode_image(bytes: &[u8]) -> Option<RawProgramWithSymbols> {
    std::panic::catch_unwind(AssertUnwindSafe(|| {
        let image: SerializableRawProgramWithSymbols =
            artifact_options().deserialize(bytes).ok()?;
        Some(image.into_raw_program())
    }))
    .ok()
    .flatten()
}

fn manifest_is_current(
    manifest: &InitCacheManifest,
    key: &CacheKey,
    init_path: &Path,
    exe_path: &Path,
    observe: &dyn Fn(&Path) -> Option<SourceStamp>,
) -> bool {
    manifest.hx_build == key.hx_build
        && manifest.steel_compat == key.steel_compat
        && manifest.steel_home == key.steel_home
        && manifest.init_script.path == init_path
        && manifest.hx_binary.path == exe_path
        && stamp_current(&manifest.hx_binary, observe)
        && stamp_current(&manifest.init_script, observe)
        && manifest
            .modules
            .iter()
            .all(|stamp| stamp_current(stamp, observe))
}

fn stamp_current(stamp: &SourceStamp, observe: &dyn Fn(&Path) -> Option<SourceStamp>) -> bool {
    observe(&stamp.path).is_some_and(|observed| observed == *stamp)
}

fn observe_stamp(path: &Path) -> Option<SourceStamp> {
    let metadata = fs::metadata(path).ok()?;
    let (mtime_secs, mtime_nanos) = mtime_parts(metadata.modified().ok()?)?;
    Some(SourceStamp {
        path: path.to_path_buf(),
        mtime_secs,
        mtime_nanos,
        size: metadata.len(),
    })
}

fn mtime_parts(mtime: SystemTime) -> Option<(u64, u32)> {
    let since_epoch = mtime.duration_since(UNIX_EPOCH).ok()?;
    Some((since_epoch.as_secs(), since_epoch.subsec_nanos()))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn artifact_options() -> impl Options {
    bincode::options().with_limit(ARTIFACT_SIZE_LIMIT)
}

fn persist(
    engine: &Engine,
    program: RawProgramWithSymbols,
    init_path: &Path,
    paths: &CachePaths,
    key: &CacheKey,
) {
    if let Err(message) = try_persist(engine, program, init_path, paths, key) {
        log::warn!("Skipping steel init cache write: {}", message);
    }
}

fn try_persist(
    engine: &Engine,
    program: RawProgramWithSymbols,
    init_path: &Path,
    paths: &CachePaths,
    key: &CacheKey,
) -> Result<(), String> {
    let exe_path =
        std::env::current_exe().map_err(|e| format!("resolving hx binary path: {}", e))?;
    let hx_binary =
        observe_stamp(&exe_path).ok_or_else(|| format!("unable to stat {:?}", exe_path))?;
    let init_script =
        observe_stamp(init_path).ok_or_else(|| format!("unable to stat {:?}", init_path))?;

    let mut modules = Vec::new();
    let metadata = engine.module_metadata();
    for (path, compiled_mtime) in metadata.iter() {
        let stamp = observe_stamp(path).ok_or_else(|| format!("unable to stat {:?}", path))?;
        let recorded = mtime_parts(*compiled_mtime)
            .ok_or_else(|| format!("pre-epoch mtime for {:?}", path))?;
        if (stamp.mtime_secs, stamp.mtime_nanos) != recorded {
            return Err(format!("{:?} changed during startup", path));
        }
        modules.push(stamp);
    }
    modules.sort_by(|a, b| a.path.cmp(&b.path));

    let serializable = program
        .into_serializable_program()
        .map_err(|e| format!("serializing program: {}", e))?;
    let artifact = artifact_options()
        .serialize(&serializable)
        .map_err(|e| format!("encoding program: {}", e))?;

    if decode_image(&artifact).is_none() {
        return Err("artifact failed round-trip validation".to_string());
    }

    let manifest = InitCacheManifest {
        hx_build: key.hx_build.clone(),
        steel_compat: key.steel_compat.clone(),
        steel_home: key.steel_home.clone(),
        hx_binary,
        init_script,
        modules,
        artifact_len: artifact.len() as u64,
        artifact_checksum: fnv1a(&artifact),
    };
    let manifest_bytes =
        serde_json::to_vec(&manifest).map_err(|e| format!("encoding manifest: {}", e))?;

    let dir = paths
        .artifact
        .parent()
        .ok_or_else(|| format!("no parent directory for {:?}", paths.artifact))?;
    fs::create_dir_all(dir).map_err(|e| format!("creating {:?}: {}", dir, e))?;
    write_atomic(&paths.artifact, &artifact)?;
    write_atomic(&paths.manifest, &manifest_bytes)?;
    log::info!(
        "Wrote steel init cache: {:?} ({} bytes)",
        paths.artifact,
        artifact.len()
    );
    Ok(())
}

fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut tmp_name = target.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    fs::write(&tmp, bytes).map_err(|e| format!("writing {:?}: {}", tmp, e))?;
    fs::rename(&tmp, target).map_err(|e| format!("installing {:?}: {}", target, e))
}

fn discard(paths: &CachePaths) {
    for path in [&paths.artifact, &paths.manifest] {
        if let Err(e) = fs::remove_file(path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Unable to discard steel init cache {:?}: {}", path, e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use steel::compiler::constants::ConstantMap;

    fn stamp(path: &str, mtime_secs: u64, mtime_nanos: u32, size: u64) -> SourceStamp {
        SourceStamp {
            path: PathBuf::from(path),
            mtime_secs,
            mtime_nanos,
            size,
        }
    }

    fn key() -> CacheKey {
        CacheKey {
            hx_build: "hx-test-build".to_string(),
            steel_compat: "steel-test".to_string(),
            steel_home: "/home/user/.steel".to_string(),
        }
    }

    fn manifest() -> InitCacheManifest {
        InitCacheManifest {
            hx_build: "hx-test-build".to_string(),
            steel_compat: "steel-test".to_string(),
            steel_home: "/home/user/.steel".to_string(),
            hx_binary: stamp("/usr/local/bin/hx", 50, 0, 90000),
            init_script: stamp("/cfg/helix/init.scm", 100, 5, 40),
            modules: vec![
                stamp("/steel/cogs/nothelix/a.scm", 200, 0, 1000),
                stamp("/steel/cogs/nothelix/b.scm", 300, 9, 2000),
            ],
            artifact_len: 0,
            artifact_checksum: 0,
        }
    }

    fn disk(stamps: &[SourceStamp]) -> impl Fn(&Path) -> Option<SourceStamp> {
        let map: HashMap<PathBuf, SourceStamp> =
            stamps.iter().map(|s| (s.path.clone(), s.clone())).collect();
        move |path: &Path| map.get(path).cloned()
    }

    fn full_disk(manifest: &InitCacheManifest) -> Vec<SourceStamp> {
        let mut stamps = manifest.modules.clone();
        stamps.push(manifest.init_script.clone());
        stamps.push(manifest.hx_binary.clone());
        stamps
    }

    #[test]
    fn unchanged_sources_validate() {
        let manifest = manifest();
        let observe = disk(&full_disk(&manifest));
        assert!(manifest_is_current(
            &manifest,
            &key(),
            Path::new("/cfg/helix/init.scm"),
            Path::new("/usr/local/bin/hx"),
            &observe
        ));
    }

    #[test]
    fn touched_module_invalidates() {
        let manifest = manifest();
        let mut stamps = full_disk(&manifest);
        stamps[0].mtime_nanos += 1;
        let observe = disk(&stamps);
        assert!(!manifest_is_current(
            &manifest,
            &key(),
            Path::new("/cfg/helix/init.scm"),
            Path::new("/usr/local/bin/hx"),
            &observe
        ));
    }

    #[test]
    fn resized_module_invalidates() {
        let manifest = manifest();
        let mut stamps = full_disk(&manifest);
        stamps[1].size += 7;
        let observe = disk(&stamps);
        assert!(!manifest_is_current(
            &manifest,
            &key(),
            Path::new("/cfg/helix/init.scm"),
            Path::new("/usr/local/bin/hx"),
            &observe
        ));
    }

    #[test]
    fn deleted_module_invalidates() {
        let manifest = manifest();
        let mut stamps = full_disk(&manifest);
        stamps.remove(0);
        let observe = disk(&stamps);
        assert!(!manifest_is_current(
            &manifest,
            &key(),
            Path::new("/cfg/helix/init.scm"),
            Path::new("/usr/local/bin/hx"),
            &observe
        ));
    }

    #[test]
    fn edited_init_script_invalidates() {
        let manifest = manifest();
        let mut stamps = full_disk(&manifest);
        let init = stamps
            .iter()
            .position(|s| s.path == manifest.init_script.path)
            .unwrap();
        stamps[init].mtime_secs += 1;
        let observe = disk(&stamps);
        assert!(!manifest_is_current(
            &manifest,
            &key(),
            Path::new("/cfg/helix/init.scm"),
            Path::new("/usr/local/bin/hx"),
            &observe
        ));
    }

    #[test]
    fn rebuilt_binary_invalidates() {
        let manifest = manifest();
        let mut stamps = full_disk(&manifest);
        let binary = stamps
            .iter()
            .position(|s| s.path == manifest.hx_binary.path)
            .unwrap();
        stamps[binary].size += 1024;
        let observe = disk(&stamps);
        assert!(!manifest_is_current(
            &manifest,
            &key(),
            Path::new("/cfg/helix/init.scm"),
            Path::new("/usr/local/bin/hx"),
            &observe
        ));
    }

    #[test]
    fn relocated_init_script_invalidates() {
        let manifest = manifest();
        let observe = disk(&full_disk(&manifest));
        assert!(!manifest_is_current(
            &manifest,
            &key(),
            Path::new("/elsewhere/init.scm"),
            Path::new("/usr/local/bin/hx"),
            &observe
        ));
    }

    #[test]
    fn relocated_binary_invalidates() {
        let manifest = manifest();
        let observe = disk(&full_disk(&manifest));
        assert!(!manifest_is_current(
            &manifest,
            &key(),
            Path::new("/cfg/helix/init.scm"),
            Path::new("/nix/store/other/bin/hx"),
            &observe
        ));
    }

    #[test]
    fn hx_build_change_invalidates() {
        let manifest = manifest();
        let observe = disk(&full_disk(&manifest));
        let mut key = key();
        key.hx_build = "hx-other-build".to_string();
        assert!(!manifest_is_current(
            &manifest,
            &key,
            Path::new("/cfg/helix/init.scm"),
            Path::new("/usr/local/bin/hx"),
            &observe
        ));
    }

    #[test]
    fn steel_rev_change_invalidates() {
        let manifest = manifest();
        let observe = disk(&full_disk(&manifest));
        let mut key = key();
        key.steel_compat = "steel-other".to_string();
        assert!(!manifest_is_current(
            &manifest,
            &key,
            Path::new("/cfg/helix/init.scm"),
            Path::new("/usr/local/bin/hx"),
            &observe
        ));
    }

    #[test]
    fn steel_home_change_invalidates() {
        let manifest = manifest();
        let observe = disk(&full_disk(&manifest));
        let mut key = key();
        key.steel_home = "/other/.steel".to_string();
        assert!(!manifest_is_current(
            &manifest,
            &key,
            Path::new("/cfg/helix/init.scm"),
            Path::new("/usr/local/bin/hx"),
            &observe
        ));
    }

    #[test]
    fn fnv1a_known_vectors() {
        assert_eq!(fnv1a(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a(b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn pre_epoch_mtime_rejected() {
        let before_epoch = UNIX_EPOCH - std::time::Duration::from_secs(1);
        assert_eq!(mtime_parts(before_epoch), None);
        assert_eq!(
            mtime_parts(UNIX_EPOCH + std::time::Duration::new(12, 34)),
            Some((12, 34))
        );
    }

    #[test]
    fn manifest_round_trips_through_json() {
        let manifest = manifest();
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let decoded: InitCacheManifest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, manifest);
    }

    #[test]
    fn corrupt_artifact_bytes_fail_decode() {
        assert!(decode_image(&[0xde, 0xad, 0xbe, 0xef]).is_none());
        assert!(decode_image(&[]).is_none());
        assert!(decode_image(&[0xff; 64]).is_none());
    }

    #[test]
    fn artifact_round_trips_through_steel_serialization() {
        let program =
            RawProgramWithSymbols::new(Vec::new(), ConstantMap::new(), "test-version".to_string());
        let bytes = artifact_options()
            .serialize(&program.into_serializable_program().unwrap())
            .unwrap();

        assert!(decode_image(&bytes).is_some());
    }
}
