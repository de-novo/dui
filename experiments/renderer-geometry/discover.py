"""Research literal Director routes and nested React Router configuration, without execution."""
import argparse
import json
from pathlib import Path

from tree_sitter import Language, Parser, Query, QueryCursor
import tree_sitter_typescript

LANG = Language(tree_sitter_typescript.language_tsx())


def parse(path):
    root = Parser(LANG).parse(path.read_bytes()).root_node
    if root.has_error:
        raise ValueError(f"invalid TSX in {path}")
    return root


def captures(node, query, name):
    return QueryCursor(Query(LANG, query)).captures(node).get(name, [])


def text(node):
    return node.text.decode()


def literal(node):
    if node.type == "string" and all(c.type == "string_fragment" for c in node.named_children):
        return text(node)[1:-1]
    return None


def fields(obj):
    if obj.type != "object":
        raise ValueError("expected literal object")
    result = {}
    for pair in obj.named_children:
        if pair.type != "pair":
            raise ValueError("unsupported object member")
        key = pair.child_by_field_name("key")
        name = literal(key) if key.type == "string" else text(key)
        if name in result:
            raise ValueError("duplicate key")
        result[name] = pair.child_by_field_name("value")
    return result


def declared_routes(router, config):
    root = parse(router)
    calls = captures(root, '(call_expression function: (identifier) @name arguments: (arguments) @args)', 'args')
    routes = {}
    constants = {}
    if config:
        matches = captures(parse(config), '(variable_declarator name: (identifier) @name value: (as_expression (object) @obj) (#eq? @name "paths"))', 'obj')
        if len(matches) != 1:
            raise ValueError("expected one paths object")
        def collect(obj, prefix):
            for name, val in fields(obj).items():
                key = prefix + "." + name
                if val.type == "object":
                    collect(val, key)
                elif literal(val) is not None:
                    constants[key] = literal(val)
        collect(matches[0], "paths")

    def visit(array, parent):
        for item in array.named_children:
            props = fields(item)
            path = props.get("path")
            if path is None:
                continue
            value = literal(path) if path.type == "string" else constants.get(text(path))
            if value == "*" or value is None:
                continue
            route = value if value.startswith("/") else (parent.rstrip("/") + "/" + value).rstrip("/") or "/"
            routes.setdefault(route, []).append(path.start_point.row + 1)
            if "children" in props and props["children"].type == "array":
                visit(props["children"], route)

    for args in calls:
        name = text(args.parent.child_by_field_name("function"))
        values = args.named_children
        if len(values) != 1:
            continue
        if name == "Router" and values[0].type == "object":
            for path, handler in fields(values[0]).items():
                if path.startswith("/"):
                    routes.setdefault(path, []).append(handler.start_point.row + 1)
        if name == "createBrowserRouter" and values[0].type == "array":
            visit(values[0], "/")
    return routes


parser = argparse.ArgumentParser()
parser.add_argument("--experimental-router", required=True, type=Path)
parser.add_argument("--paths", type=Path, help="resolved config module for the paths binding")
args = parser.parse_args()
print(json.dumps(declared_routes(args.experimental_router, args.paths), sort_keys=True))
