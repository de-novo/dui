import { h, render } from "preact";
import { LocationProvider, Router, Route } from "preact-iso";
import pages from "./pages.json";
import "../screen.css";

function Screen({ page }) {
  const next = pages.find((entry) => entry.path === page.next);
  return h("main", null, [
    h("h1", null, page.heading),
    h("p", null, page.body),
    h("a", { href: next.path }, next.heading),
  ]);
}

const routes = pages.map((page) => h(Route, { path: page.path, component: Screen, page }));
render(h(LocationProvider, null, h(Router, null, routes)), document.querySelector("#app"));
