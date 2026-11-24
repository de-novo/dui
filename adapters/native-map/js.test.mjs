import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { discover } from "./discover-js.mjs";

const require = createRequire(import.meta.url);
function fixture(t, files, dependencies) {
  const root = mkdtempSync(join(tmpdir(), "dui-native-js-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), text);
  }
  writeFileSync(join(root, "package.json"), JSON.stringify({ dependencies }));
  // Resolve from this fixture; the adapter itself must always use the project's compiler.
  mkdirSync(join(root, "node_modules"));
  symlinkSync(
    dirname(require.resolve("typescript/package.json")),
    join(root, "node_modules", "typescript"),
    "junction",
  );
  return root;
}

test("Expo route groups, dynamic index, default exports, variants and exclusions", async (t) => {
  const view = "export default function Page() { return null }";
  const root = fixture(
    t,
    {
      "src/app/(private)/user/[id]/index.tsx": view,
      "src/app/(public)/user/[id]/index.tsx": "export { default } from './component'",
      "src/app/_layout.tsx": view,
      "src/app/_layout.ios.tsx": view,
      "src/app/+native-intent.ts": "export function redirectSystemPath() {}",
      "src/app/orders+api.ts": "export function GET() {}",
      "src/app/+not-found.tsx": view,
      "src/app/settings.tsx": "\n" + view,
      "src/app/settings.ios.tsx": view,
      "src/app/settings.web.tsx": view,
      "src/app/stray.android.tsx": view,
      "src/app/data.ts": "export const x = 17",
      "app/unused.tsx": view,
    },
    { "expo-router": "*" },
  );
  const result = await discover(root);
  assert.deepEqual(
    result.screens.map((s) => [s.name, s.path, s.dynamic ?? false]),
    [
      ["/+not-found", "/+not-found", false],
      ["/settings", "/settings", false],
      ["/(private)/user/[id]", "/user/[id]", true],
      ["/(public)/user/[id]", "/user/[id]", true],
    ],
  );
  assert.equal(result.screens[1].line, 2);
  assert.ok(result.limitations.some((m) => m.includes("data.ts")));
  assert.ok(result.limitations.some((m) => m.includes("stray")));
});

test("React Navigation declarations require navigator ownership and preserve conditional scopes", async (t) => {
  const root = fixture(
    t,
    {
      "stack.ts":
        "export { createNativeStackNavigator as make } from '@react-navigation/native-stack'",
      "Routes.tsx": `
import { make } from './stack'
const extra = { Profile: { screen: Profile, linking: { path: 'people/:id' } } } as const
const menu = make({
  screens: {
    ...extra,
    ...(flag ? { Preview: Preview } : { Published }),
    ...customScreens(),
  },
  groups: { Account: { if: isLoggedIn, screens: { Preferences } } },
})
const Stack = make()
const Other = unrelatedNavigator()
export function Screens() {
  return <Stack.Navigator>
    <Stack.Screen name="Home" component={Home}/>
    {enabled && <Stack.Group><Stack.Screen name={'Activity'} component={Feed}/></Stack.Group>}
    <Other.Screen name="Outside"/>
    <Stack.Screen name={runtimeName}/>
  </Stack.Navigator>
}
export const unrelated = <Stack.Screen name="Stray"/>
const sample = { screens: { Imaginary: Foo } }
`,
    },
    { "@react-navigation/native-stack": "*" },
  );
  const result = await discover(root);
  assert.deepEqual(
    result.screens.map((s) => [s.name, s.path, s.condition ?? null, s.dynamic ?? false]),
    [
      ["Profile", "/people/:id", null, true],
      ["Activity", "Activity", "enabled", false],
      ["Home", "Home", null, false],
      ["Preferences", "Preferences", "isLoggedIn", false],
      ["Preview", "Preview", "flag", false],
      ["Published", "Published", "!(flag)", false],
    ],
  );
  assert.ok(result.screens.every((s) => s.source === "Routes.tsx" && s.line > 1));
  assert.ok(result.limitations.some((m) => m.includes("Registration object")));
  assert.ok(result.limitations.some((m) => m.includes("Dynamic screen name")));
});
