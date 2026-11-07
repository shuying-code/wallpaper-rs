//! 构建脚本：为「单文件自带 mpv」发行版（`bundled-mpv` feature）把 mpv 预压缩后内嵌。
//!
//! 启用该 feature 时，把 `bundled-mpv/mpv.exe` 与 `bundled-mpv/d3dcompiler_43.dll`
//! 用 gzip 压缩后写入 `OUT_DIR`，供 `crate::livewall` 用 `include_bytes!` 内嵌；
//! 运行时再按需解压到用户缓存目录。直接内嵌未压缩的 mpv 会让单文件 exe 达到 ~130 MB，
//! 压缩后约 45 MB，显著减小体积。
//!
//! 未启用该 feature 时脚本不做任何事（对应代码也不会被编译）。

use std::io::Write;

fn main() {
    // mpv 文件变化时才重新压缩；feature 开关变化时也重跑。
    println!("cargo:rerun-if-changed=bundled-mpv/mpv.exe");
    println!("cargo:rerun-if-changed=bundled-mpv/d3dcompiler_43.dll");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_BUNDLED_MPV");

    if std::env::var_os("CARGO_FEATURE_BUNDLED_MPV").is_none() {
        return;
    }

    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR 未设置"));
    for (src, out_name, len_env) in [
        ("bundled-mpv/mpv.exe", "mpv.exe.gz", "WPRS_MPV_LEN"),
        (
            "bundled-mpv/d3dcompiler_43.dll",
            "d3dcompiler_43.dll.gz",
            "WPRS_DLL_LEN",
        ),
    ] {
        let data = std::fs::read(src)
            .unwrap_or_else(|e| panic!("读取 {src} 失败：{e}（bundled-mpv 需先放入 mpv 文件）"));
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        enc.write_all(&data).expect("gzip 压缩失败");
        let gz = enc.finish().expect("gzip 结束失败");
        std::fs::write(out.join(out_name), &gz).expect("写入压缩数据失败");
        // 把原始大小传给运行时，用于判断缓存文件是否已就绪（避免每次启动都解压）。
        println!("cargo:rustc-env={len_env}={}", data.len());
    }
}
