import type { NextConfig } from "next";

/**
 * In development the web app and the API run as separate processes. The
 * rewrite keeps the browser talking to a single origin, so cookies stay
 * same-origin and no CORS exception is needed.
 */
const apiOrigin = process.env.HOMECLOUD_API_ORIGIN ?? "http://127.0.0.1:8080";

/**
 * Baseline response headers for the web app.
 *
 * `'unsafe-inline'` for scripts is required by the framework's hydration
 * payload; tightening it to a per-request nonce needs middleware and is
 * tracked as follow-up work rather than left implied.
 */
const SECURITY_HEADERS = [
  {
    key: "Content-Security-Policy",
    value: [
      "default-src 'self'",
      "script-src 'self' 'unsafe-inline'",
      "style-src 'self' 'unsafe-inline'",
      "img-src 'self' blob: data:",
      "font-src 'self'",
      "connect-src 'self'",
      "object-src 'none'",
      "base-uri 'none'",
      "form-action 'self'",
      "frame-ancestors 'none'",
    ].join("; "),
  },
  { key: "X-Content-Type-Options", value: "nosniff" },
  { key: "Referrer-Policy", value: "no-referrer" },
  { key: "X-Frame-Options", value: "DENY" },
  {
    key: "Permissions-Policy",
    value: "camera=(), microphone=(), geolocation=(), interest-cohort=()",
  },
];

const nextConfig: NextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  async headers() {
    return [{ source: "/:path*", headers: SECURITY_HEADERS }];
  },
  async rewrites() {
    return [
      { source: "/api/:path*", destination: `${apiOrigin}/api/:path*` },
      { source: "/health/:path*", destination: `${apiOrigin}/health/:path*` },
    ];
  },
};

export default nextConfig;
