import { createApp, h } from "vue";
import { createRouter, createWebHistory, RouterLink, RouterView } from "vue-router";
import pages from "./pages.json";
import "../screen.css";

const routes = pages.map((page) => ({
  path: page.path,
  component: {
    render() {
      const next = pages.find((entry) => entry.path === page.next);
      return h("main", [
        h("h1", page.heading),
        h("p", page.body),
        h(RouterLink, { to: next.path }, () => next.heading),
      ]);
    },
  },
}));

const router = createRouter({ history: createWebHistory(), routes });
createApp(RouterView).use(router).mount("#app");
