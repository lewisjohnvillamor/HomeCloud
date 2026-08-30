import coreWebVitals from "eslint-config-next/core-web-vitals";
import typescript from "eslint-config-next/typescript";

const config = [
  {
    ignores: [
      ".next/**",
      "node_modules/**",
      "playwright-report/**",
      "test-results/**",
      "next-env.d.ts",
    ],
  },
  ...coreWebVitals,
  ...typescript,
  {
    rules: {
      eqeqeq: ["error", "always"],
      "no-console": ["error", { allow: ["warn", "error"] }],
    },
  },
];

export default config;
