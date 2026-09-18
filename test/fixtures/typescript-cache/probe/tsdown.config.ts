export default {
  entry: ["src/index.ts", "src/action.ts"],
  format: ["esm"],
  dts: false,
  target: "node26",
  noExternal: [/.*/],
};
