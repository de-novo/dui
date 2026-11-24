// Compare project-declared tag/property pairs against captured computed styles.
import { readFileSync } from "node:fs";

function computedValue(value) {
  if (!/^#[0-9a-f]{6}$/i.test(value)) return value;
  const channels = value
    .slice(1)
    .match(/../g)
    .map((channel) => parseInt(channel, 16));
  return `rgb(${channels.join(", ")})`;
}

const { renderTree, designSpec } = JSON.parse(readFileSync(0, "utf8"));
const { rules, tokens } = designSpec.content;
if (!Array.isArray(rules) || !rules.length) throw new Error("expected at least one token rule");
const findings = [];
const unverified = [];
for (const [index, rule] of rules.entries()) {
  const { tag, property, token } = rule;
  const expected = tokens[token];
  if (
    ![tag, property, token, expected].every((value) => typeof value === "string" && value.trim())
  ) {
    throw new Error(`invalid token rule at index ${index}`);
  }
  const nodes = renderTree.filter((node) => node.tag === tag);
  if (!nodes.length) unverified.push(`No captured <${tag}> for rule ${index}`);
  for (const node of nodes) {
    const measured = node.style?.[property];
    if (typeof measured !== "string") {
      unverified.push(`No captured ${property} for ${node.id}`);
    } else if (measured !== computedValue(expected)) {
      findings.push({
        rule: `${tag}.${property}`,
        severity: "error",
        nodeId: node.id,
        rect: node.rect,
        summary: `${property} differs from the ${token} token`,
        measured,
        expected,
        specRef: `${designSpec.path}#/rules/${index}`,
      });
    }
  }
}
console.log(JSON.stringify({ findings, unverified }));
