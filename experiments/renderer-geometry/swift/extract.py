"""One-shape SwiftUI feasibility probe, run with --experimental-swift."""
import argparse
import hashlib
import json
from pathlib import Path

import jsonschema
from tree_sitter import Language, Parser, Query, QueryCursor
import tree_sitter_swift

args = argparse.ArgumentParser()
args.add_argument("--experimental-swift", required=True, action="store_true")
args.add_argument("source", type=Path)
args.add_argument("schema", type=Path)
options = args.parse_args()
source = options.source.read_bytes()
language = Language(tree_sitter_swift.language())
tree = Parser(language).parse(source)
query = Query(language, """
(class_declaration "struct" name: (type_identifier) @name
 (inheritance_specifier inherits_from: (user_type (type_identifier) @conformance))
 body: (class_body (property_declaration name: (pattern (simple_identifier) @body)
 computed_value: (computed_property (statements .
 (call_expression
   (navigation_expression target: (call_expression
       (simple_identifier) @view
       (call_suffix (value_arguments . (value_argument value: (line_string_literal . (line_str_text) @text .)) .)))
     suffix: (navigation_suffix (simple_identifier) @modifier))
   (call_suffix (value_arguments . (value_argument value: (integer_literal) @length) .))) @call .))))
) @declaration
""")
matches = QueryCursor(query).matches(tree.root_node)
if tree.root_node.has_error or len(matches) != 1:
    raise ValueError("unsupported Swift syntax")
captures = matches[0][1]

def token(key):
    return captures[key][0].text.decode()

if [token(k) for k in ("conformance", "body", "view", "modifier")] != ["View", "body", "Text", "padding"]:
    raise ValueError("unsupported view or modifier")

def location(key):
    node = captures[key][0]
    return dict(file=options.source.name, line=node.start_point.row+1,
                column=node.start_point.column+1, start=node.start_byte, end=node.end_byte)

name = token("name")
component = options.source.name + "#" + name
root = dict(kind="element", id=component+"@0", tag=token("view"), location=location("call"), children=[
    dict(kind="text", id=component+"@1", location=location("text"), parts=[dict(kind="literal", value=token("text"))])
], style=[dict(property="padding"+edge, value=dict(kind="length", value=int(token("length")), unit="pt"),
               **{"from": dict(syntax="modifier", text=token("call"), location=location("call"))})
          for edge in ("Top", "Bottom", "Start", "End")])
model = dict(schemaVersion=1, extractor=dict(name="swift-syntax-probe", version="0.1.0"),
             files=[dict(path=options.source.name, language="swift", sha256=hashlib.sha256(source).hexdigest())],
             screens=[dict(id=component, name=name, component=component, origin="syntax", location=location("declaration"))],
             components=[dict(id=component, name=name, root=root, location=location("declaration"))])
jsonschema.Draft202012Validator(json.loads(options.schema.read_text())).validate(model)
print(json.dumps(model, ensure_ascii=False))
