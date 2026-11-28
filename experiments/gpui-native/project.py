"""Watch a known SwiftUI or Compose fixture and publish supported literal Text nodes."""

import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

import tree_sitter_kotlin
from tree_sitter import Language, Parser
from watchfiles import watch


def labels(source, name):
    if source.suffix == ".swift":
        dump = subprocess.check_output(
            ["xcrun", "swiftc", "-frontend", "-dump-parse", str(source)], text=True
        )
        block = next(
            b
            for b in dump.split("\n  (struct_decl ")
            if f'"{name}" inherits=' in b.splitlines()[0]
        )
        texts = []
        lines = block.splitlines()
        for i, line in enumerate(lines):
            if 'name="Text" function_ref=' in line:
                value = re.search(
                    r'string_literal_expr[^\n]*value="([^"]*)"',
                    "\n".join(lines[i + 1 : i + 9]),
                )
                if not value:
                    raise ValueError("unsupported Text argument")
                texts.append(value[1])
        return texts
    tree = Parser(Language(tree_sitter_kotlin.language())).parse(source.read_bytes())
    if tree.root_node.has_error:
        raise ValueError("invalid Kotlin")
    fn = next(
        n
        for n in tree.root_node.named_children
        if n.type == "function_declaration"
        and n.child_by_field_name("name").text.decode() == name
    )

    def visit(n):
        if n.type == "call_expression" and n.named_children[0].text == b"Text":
            arg = n.named_children[1].named_children[0].named_children[0]
            if arg.type != "string_literal":
                raise ValueError("unsupported Text argument")
            yield arg.named_children[0].text.decode()
        else:
            for c in n.named_children:
                yield from visit(c)

    return list(visit(fn))


if __name__ == "__main__":
    source, field, output = sys.argv[1:]
    source = Path(source).resolve()
    output = Path(output)
    last = None
    revision = 0

    def publish():
        global last, revision
        current = source.read_bytes()
        if current == last:
            return
        start = time.monotonic_ns()
        text = labels(source, field)
        if len(text) != (4 if field == "StackExample" else 1):
            raise ValueError(text)
        tmp = output.with_suffix(".tmp")
        tmp.write_text(
            json.dumps(
                {"stack": field == "StackExample", "labels": text, "revision": revision}
            )
        )
        os.replace(tmp, output)
        print(
            json.dumps(
                {"revision": revision, "projectMs": (time.monotonic_ns() - start) / 1e6}
            ),
            flush=True,
        )
        last = current
        revision += 1

    publish()
    for _ in watch(
        source.parent,
        watch_filter=lambda change, p: Path(p) == source,
        debounce=12,
        step=4,
    ):
        publish()
