import { error } from "@sveltejs/kit";
import pages from "../../../pages.json";

export const prerender = true;

export function load({ url }) {
  const page = pages.find((entry) => entry.path === url.pathname);
  if (!page) error(404, "Unknown trail");
  return { page, next: pages.find((entry) => entry.path === page.next) };
}
