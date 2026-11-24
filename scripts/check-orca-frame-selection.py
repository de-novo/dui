"""Check delayed rendering with the two real simulator PNGs from the capture job."""

from pathlib import Path
import runpy
import sys
import tempfile
from unittest.mock import Mock


def main(captures):
    capture = runpy.run_path(str(Path(__file__).with_name("check-ios-orca.py")))
    with tempfile.TemporaryDirectory() as directory:
        containers = Path(directory)
        documents = containers / "runner" / "Documents"
        documents.mkdir(parents=True)
        for _, name, witness in capture["SCREENS"]:
            target = captures / f"orca-{name}.png"
            other, = (image for image in captures.glob("orca-*.png") if image != target)
            expected = target.read_bytes()
            frames = iter((other.read_bytes(), expected))
            candidate = documents / f"orca-{name}.candidate.png"
            accepted = documents / f"orca-{name}.accepted"

            def next_frame():
                if accepted.exists():
                    raise AssertionError(f"Unexpected acknowledgement before {name} was visible")
                candidate.write_bytes(next(frames))

            process = Mock()
            process.poll.side_effect = next_frame
            result = capture["wait_for_visible_frame"](containers, process, name, witness)
            if result != expected or not accepted.exists():
                raise AssertionError(f"Expected the second frame for {name}")
    print("Both routes rejected the wrong screenshot and accepted the matching pixels")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
