"""Syntax-only projection of one literal Text call. Reject other shapes."""
from pathlib import Path
import json,sys
from tree_sitter import Parser,Language
import tree_sitter_kotlin
source,screen,revision=sys.argv[1:]
root=Parser(Language(tree_sitter_kotlin.language())).parse(Path(source).read_bytes()).root_node
if root.has_error:raise ValueError('parse error')
fn,=[n for n in root.named_children if n.type=='function_declaration' and n.child_by_field_name('name').text.decode()==screen]
node=next(n for n in fn.named_children if n.type=='function_body')
for kind in ['block','call_expression']:
    node,=node.named_children
    assert node.type==kind
callee,args=node.named_children
assert callee.text==b'Text' and args.type=='value_arguments'
for kind in ['value_argument','string_literal','string_content']:
    args,=args.named_children
    assert args.type==kind
print(json.dumps(dict(screen=screen,text=args.text.decode(),revision=int(revision))))
