// The little of Node's test runner the editor's tests use, so that they typecheck with the
// editor's own settings (DOM) and without @types/node.
declare module "node:test" {
  export function test(name: string, body: () => void | Promise<void>): void;
}
declare module "node:assert" {
  export const strict: {
    equal(actual: unknown, expected: unknown, message?: string): void;
  };
}
