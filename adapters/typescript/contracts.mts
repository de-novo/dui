// The host loads the target project's compiler. The extractor only consumes
// this common view of TS6 nodes and TS7 nodes resolved from native handles.
import type { Node } from "typescript/unstable/ast";
import type { Type } from "typescript/unstable/sync";

export type Syntax = Node & {
  text: string;
  name: Syntax;
  parent: Syntax;
  initializer: Syntax;
  expression: Syntax;
  type: Syntax;
  condition: Syntax;
  operatorToken: Syntax;
  operator: number;
  operand: Syntax;
  left: Syntax;
  right: Syntax;
  thenStatement: Syntax;
  elseStatement?: Syntax;
  whenTrue: Syntax;
  whenFalse: Syntax;
  tagName: Syntax;
  attributes?: { properties?: Syntax[] };
  moduleSpecifier?: Syntax;
  arguments: Syntax[];
  elements: Syntax[];
  children: Syntax[];
  properties?: Syntax[];
  parameters?: Syntax[];
  body?: Syntax;
  statements?: Syntax[];
};
export type { Type };
export interface Scope {
  tsconfig?: string;
  sourceDirs?: string[];
  sources?: string[];
}
export interface Manifest extends Scope {
  additionalScopes?: Scope[];
  appRoot?: string;
  host?: string;
  rulePacks?: string[];
}
export interface Rule {
  kind: string;
  package: string;
  symbol: string;
}
export type Shape =
  | { kind: "unknown" | "boolean" | "number" | "string" }
  | { kind: "literal"; value: unknown }
  | { kind: "union"; members: Shape[] }
  | { kind: "array"; item: Shape }
  | { kind: "object"; fields: { name: string; shape: Shape }[] };
export interface Fact {
  file: string;
  start: number;
  end: number;
  kind: string;
  [field: string]: unknown;
}
