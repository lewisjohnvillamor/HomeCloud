import type { NextConfig } from "next";

/**
 * In development the web app and the API run as separate processes. The
 * rewrite keeps the browser talking to a single origin, so cookies stay
 * same-origin and no CORS exception is needed.
 */
const apiOrigin = process.env.HOMECLOUD_API_ORIGIN ?? "http://127.0.0.1:8080";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  async rewrites() {
    return [
      { source: "/api/:path*", destination: `${apiOrigin}/api/:path*` },
      { source: "/health/:path*", destination: `${apiOrigin}/health/:path*` },
    ];
  },
};

export default nextConfig;
