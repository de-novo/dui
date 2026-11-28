## Fractional flex layout on Linux

The GPUI native layout candidate was exercised independently of font measurement and window painting. Both shards ran concurrently on default Boat Linux x86_64 VMs. One built Taffy 0.15.1 with rustc 1.97.1 in release mode, passed one unit test, and emitted rectangles. The other used Google Chrome 151.0.7922.108 stable to run the optional CSS reference capture, compare all rectangles and capture the visual examples. Boat reported 103 seconds for the Rust VM and 138 seconds for the web VM. No latency comparison was made.

| Root width | Taffy rounding | Cases within 1 CSS px | Cases within 0.25 CSS px | Maximum component error |
|---|---|---:|---:|---:|
| 390 | off | 144/144 | 144/144 | 0.0129 px |
| 390 | on | 144/144 | 0/144 | 0.96875 px |
| 1440 | off | 144/144 | 144/144 | 0.0129 px |
| 1440 | on | 144/144 | 0/144 | 0.96875 px |

Use fractional layout coordinates up to the native paint boundary when fractional geometry matters. These 288 deterministic four-box cases cross two root widths, four container caps, three gaps, wrapping on/off, three justification policies and both row directions. Each case takes the maximum error over 16 coordinates or dimensions against getBoundingClientRect. The oracle sets the root's CSS width explicitly. The pictures illustrate one wrapped case per width using Chrome to paint both the native CSS reference and the rectangles returned by Rust. They do not measure native rasterization or SwiftUI/Compose parity.

The adapter uses only Taffy (MIT), with default features off and taffy_tree and flexbox enabled. Its two transitive dependencies are slotmap (Zlib) and the build dependency version_check (MIT or Apache-2.0). GPUI 0.3.14 already depends on this Taffy version.

Reproduce from this directory on Linux with Chrome and Rust installed:

```sh
cargo test --release --locked
cargo run --quiet --release --locked > taffy.csv
google-chrome --headless --no-sandbox --disable-dev-shm-usage --dump-dom "file://$PWD/reference.html" > dom.html
python3 -c 'import pathlib,re; print(re.search(r"<pre>(.*?)</pre>",pathlib.Path("dom.html").read_text(),re.S)[1])' > reference.csv
python3 compare.py
python3 capture.py
```

The raw references and both candidate outputs are compressed CSVs in evidence. The first six columns are root width, width cap, gap, wrap flag, justification index and reverse flag. Taffy adds the rounding flag before the four child rectangles. Each rectangle is x, y, width, height in logical pixels.
