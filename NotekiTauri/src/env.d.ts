declare module "*.css";
declare module "turndown-plugin-gfm" {
  import type { Plugin } from "turndown";
  export const gfm: Plugin;
  export const tables: Plugin;
}
