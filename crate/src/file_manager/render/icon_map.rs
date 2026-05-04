//! Mapping from lowercase file extension → (icon glyph, RGBA color).
//!
//! Covers ~120 file types grouped by category. Users can override individual
//! mappings via [`FileManagerConfig::icon_override`](crate::file_manager::FileManagerConfig::icon_override).

use crate::{icons, theme};

/// Return (icon, color) for a file based on its lowercase extension.
pub(super) fn file_icon_for_ext(ext: &str) -> (&'static str, [f32; 4]) {
    match ext {
        // ── Rust ──
        "rs" | "toml" => (icons::LANGUAGE_RUST, [0.87, 0.49, 0.26, 1.0]),
        "lock" => (icons::LOCK, [0.60, 0.60, 0.60, 1.0]),

        // ── Python ──
        "py" | "pyi" | "pyw" | "pyx" | "pxd" => (icons::LANGUAGE_PYTHON, [0.30, 0.60, 0.88, 1.0]),
        "ipynb" => (icons::NOTEBOOK, [0.90, 0.55, 0.20, 1.0]),

        // ── JavaScript / TypeScript ──
        "js" | "mjs" | "cjs" | "jsx" => (icons::LANGUAGE_JAVASCRIPT, [0.95, 0.85, 0.30, 1.0]),
        "ts" | "tsx" | "mts" | "cts" => (icons::LANGUAGE_TYPESCRIPT, [0.20, 0.50, 0.85, 1.0]),

        // ── C / C++ / Objective-C ──
        "c" | "h" => (icons::LANGUAGE_C, [0.40, 0.55, 0.80, 1.0]),
        "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh" | "inl" => {
            (icons::LANGUAGE_CPP, [0.40, 0.55, 0.80, 1.0])
        }
        "m" | "mm" => (icons::APPLE, [0.60, 0.60, 0.70, 1.0]),

        // ── C# / F# ──
        "cs" | "csx" => (icons::LANGUAGE_CSHARP, [0.55, 0.35, 0.78, 1.0]),
        "fs" | "fsx" | "fsi" => (icons::LANGUAGE_CSHARP, [0.30, 0.55, 0.75, 1.0]),

        // ── Java / Kotlin / Scala / Groovy ──
        "java" | "class" | "jar" => (icons::LANGUAGE_JAVA, [0.80, 0.35, 0.25, 1.0]),
        "kt" | "kts" => (icons::LANGUAGE_KOTLIN, [0.60, 0.40, 0.85, 1.0]),
        "scala" | "sc" => (icons::FILE_CODE, [0.85, 0.30, 0.25, 1.0]),
        "groovy" | "gradle" => (icons::LANGUAGE_JAVA, [0.40, 0.60, 0.50, 1.0]),

        // ── Go ──
        "go" | "mod" | "sum" => (icons::LANGUAGE_GO, [0.00, 0.68, 0.84, 1.0]),

        // ── Swift / Dart ──
        "swift" => (icons::LANGUAGE_SWIFT, [0.95, 0.45, 0.25, 1.0]),
        "dart" => (icons::GOOGLE, [0.30, 0.70, 0.90, 1.0]),

        // ── Ruby / Perl / PHP / Lua ──
        "rb" | "rake" | "gemspec" => (icons::LANGUAGE_RUBY, [0.85, 0.20, 0.20, 1.0]),
        "pl" | "pm" | "pod" => (icons::FILE_CODE, [0.40, 0.55, 0.70, 1.0]),
        "php" | "phtml" => (icons::LANGUAGE_PHP, [0.55, 0.55, 0.80, 1.0]),
        "lua" => (icons::LANGUAGE_LUA, [0.20, 0.20, 0.80, 1.0]),

        // ── Haskell / Elixir / Erlang / R ──
        "hs" | "lhs" => (icons::LANGUAGE_HASKELL, [0.55, 0.45, 0.65, 1.0]),
        "ex" | "exs" => (icons::WATER, [0.45, 0.30, 0.60, 1.0]),
        "erl" | "hrl" => (icons::FILE_CODE, [0.70, 0.20, 0.30, 1.0]),
        "r" | "rmd" => (icons::LANGUAGE_R, [0.28, 0.48, 0.75, 1.0]),

        // ── Web ──
        "html" | "htm" | "xhtml" => (icons::LANGUAGE_HTML5, [0.90, 0.35, 0.20, 1.0]),
        "css" | "scss" | "sass" | "less" | "styl" => {
            (icons::LANGUAGE_CSS3, [0.20, 0.55, 0.85, 1.0])
        }
        "vue" => (icons::VUEJS, [0.30, 0.75, 0.55, 1.0]),
        "svelte" => (icons::FILE_CODE, [0.95, 0.30, 0.15, 1.0]),
        "wasm" => (icons::WEB, [0.40, 0.35, 0.80, 1.0]),

        // ── Data / Config ──
        "json" | "jsonc" | "json5" | "geojson" => (icons::CODE_JSON, [0.90, 0.80, 0.30, 1.0]),
        "xml" | "xsl" | "xslt" | "xsd" | "dtd" => (icons::XML, [0.85, 0.55, 0.20, 1.0]),
        "yaml" | "yml" => (icons::FILE_COG, theme::TEXT_SECONDARY),
        "ini" | "cfg" | "conf" | "properties" | "env" => {
            (icons::COG_OUTLINE, theme::TEXT_SECONDARY)
        }
        "csv" | "tsv" => (icons::FILE_DELIMITED, [0.20, 0.60, 0.30, 1.0]),
        "sql" | "sqlite" | "db" => (icons::DATABASE, [0.55, 0.45, 0.70, 1.0]),
        "graphql" | "gql" => (icons::GRAPH_OUTLINE, [0.85, 0.25, 0.55, 1.0]),
        "proto" | "protobuf" => (icons::FILE_CODE, [0.50, 0.65, 0.45, 1.0]),

        // ── Documents ──
        "pdf" => (icons::FILE_PDF_BOX, [0.85, 0.25, 0.22, 1.0]),
        "doc" | "docx" | "odt" | "rtf" => (icons::FILE_WORD, [0.25, 0.45, 0.80, 1.0]),
        "xls" | "xlsx" | "ods" => (icons::FILE_EXCEL, [0.20, 0.60, 0.30, 1.0]),
        "ppt" | "pptx" | "odp" => (icons::FILE_POWERPOINT, [0.85, 0.40, 0.20, 1.0]),
        "txt" | "log" | "readme" | "nfo" | "diz" => (icons::TEXT_BOX, theme::TEXT_SECONDARY),
        "md" | "mdx" | "rst" | "adoc" | "tex" | "latex" => {
            (icons::LANGUAGE_MARKDOWN, [0.50, 0.70, 0.90, 1.0])
        }
        "epub" | "mobi" | "azw" => (icons::BOOK_OPEN_VARIANT, [0.65, 0.50, 0.35, 1.0]),

        // ── Images ──
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" | "tiff" | "tif" | "tga"
        | "hdr" | "exr" => (icons::FILE_IMAGE, [0.45, 0.75, 0.45, 1.0]),
        "svg" => (icons::SVG, [0.90, 0.65, 0.20, 1.0]),
        "psd" | "ai" | "sketch" | "fig" | "xd" => (icons::PALETTE, [0.35, 0.65, 0.95, 1.0]),
        "blend" | "fbx" | "obj" | "stl" | "gltf" | "glb" | "3ds" | "dae" => {
            (icons::CUBE_OUTLINE, [0.80, 0.60, 0.40, 1.0])
        }

        // ── Audio ──
        "mp3" | "wav" | "ogg" | "flac" | "aac" | "wma" | "m4a" | "opus" | "ape" | "aiff" => {
            (icons::FILE_MUSIC, [0.70, 0.45, 0.80, 1.0])
        }
        "mid" | "midi" => (icons::PIANO, [0.55, 0.55, 0.70, 1.0]),

        // ── Video ──
        "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg" | "vob" => {
            (icons::FILE_VIDEO, [0.85, 0.55, 0.35, 1.0])
        }
        "srt" | "sub" | "ssa" | "ass" | "vtt" => {
            (icons::SUBTITLES_OUTLINE, [0.65, 0.65, 0.50, 1.0])
        }

        // ── Archives ──
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "lz" | "lzma" | "cab"
        | "iso" | "dmg" | "img" => (icons::ZIP_BOX, [0.75, 0.65, 0.40, 1.0]),
        "deb" | "rpm" | "pkg" | "apk" | "snap" | "flatpak" | "appimage" => {
            (icons::PACKAGE_VARIANT_CLOSED, [0.50, 0.70, 0.50, 1.0])
        }

        // ── Executables / Libraries ──
        "exe" | "msi" | "com" => (icons::APPLICATION, [0.70, 0.70, 0.70, 1.0]),
        "dll" | "so" | "dylib" | "a" | "lib" | "o" => (icons::PUZZLE, [0.60, 0.60, 0.70, 1.0]),
        "bin" | "dat" | "raw" => (icons::FILE, [0.55, 0.55, 0.55, 1.0]),

        // ── Shell scripts ──
        "sh" | "bash" | "zsh" | "fish" => (icons::CONSOLE, [0.50, 0.75, 0.50, 1.0]),
        "bat" | "cmd" | "ps1" | "psm1" => (icons::POWERSHELL, [0.30, 0.45, 0.70, 1.0]),

        // ── DevOps / CI ──
        "dockerfile" | "containerfile" => (icons::DOCKER, [0.20, 0.60, 0.85, 1.0]),
        "tf" | "hcl" => (icons::TERRAFORM, [0.40, 0.35, 0.75, 1.0]),
        "nix" => (icons::SNOWFLAKE, [0.45, 0.60, 0.85, 1.0]),

        // ── Fonts ──
        "ttf" | "otf" | "woff" | "woff2" | "eot" => (icons::FORMAT_FONT, [0.65, 0.65, 0.70, 1.0]),

        // ── Certificates / Keys ──
        "pem" | "crt" | "cer" | "key" | "p12" | "pfx" | "csr" => {
            (icons::CERTIFICATE, [0.85, 0.70, 0.25, 1.0])
        }
        "pub" | "gpg" | "asc" => (icons::KEY, [0.75, 0.65, 0.30, 1.0]),

        // ── Misc ──
        "gitignore" | "gitattributes" | "gitmodules" => (icons::GIT, [0.90, 0.35, 0.20, 1.0]),
        "editorconfig" | "prettierrc" | "eslintrc" => (icons::COG_OUTLINE, theme::TEXT_SECONDARY),
        "license" | "licence" => (icons::SCALE_BALANCE, [0.70, 0.70, 0.50, 1.0]),

        // ── Default ──
        _ => (icons::FILE, theme::ACCENT),
    }
}
