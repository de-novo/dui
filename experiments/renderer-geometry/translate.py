"""Experimental extractor for a sheet containing one static ID rule."""
import hashlib
import math

import tinycss2


def translation(model, css_bytes):
    id_value = model["components"][0]["root"]["props"]["id"]["value"]
    if not any(f["sha256"] == hashlib.sha256(css_bytes).hexdigest() and f["language"] == "css"
               for f in model["files"]):
        raise ValueError("CSS does not match a model input")
    rules, _ = tinycss2.parse_stylesheet_bytes(css_bytes, skip_comments=True, skip_whitespace=True)
    if len(rules) != 1 or rules[0].type != "qualified-rule":
        return None
    selector = [t for t in rules[0].prelude if t.type != "whitespace"]
    if len(selector) != 1 or selector[0].type != "hash" or not selector[0].is_identifier or selector[0].value != id_value:
        return None
    declarations = tinycss2.parse_blocks_contents(rules[0].content, skip_comments=True, skip_whitespace=True)
    if any(d.type != "declaration" or d.lower_name not in {"transform", "width", "height", "max-width"} for d in declarations):
        return None
    transforms = [d for d in declarations if d.lower_name == "transform"]
    if len(transforms) != 1:
        return None
    tokens = [t for t in transforms[0].value if t.type != "whitespace"]
    if len(tokens) != 1 or tokens[0].type != "function" or tokens[0].lower_name != "translate":
        return None
    args = [t for t in tokens[0].arguments if t.type != "whitespace"]
    if len(args) != 3 or args[1].type != "literal" or args[1].value != ",":
        return None
    lengths = [args[0], args[2]]
    if not all(t.type == "dimension" and t.lower_unit == "px" and math.isfinite(t.value) for t in lengths):
        return None
    return [t.value for t in lengths]
