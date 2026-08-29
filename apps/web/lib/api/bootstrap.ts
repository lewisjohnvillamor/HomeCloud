import { getJson, type ApiResult, type RequestOptions } from "./client";

export type BootstrapStatus = {
  needsOwner: boolean;
};

function parseBootstrapStatus(value: unknown): BootstrapStatus | undefined {
  if (typeof value !== "object" || value === null) {
    return undefined;
  }

  const candidate = value as Record<string, unknown>;
  if (typeof candidate.needs_owner !== "boolean") {
    return undefined;
  }

  return { needsOwner: candidate.needs_owner };
}

/** First-run status of the deployment. */
export function fetchBootstrapStatus(
  options?: RequestOptions,
): Promise<ApiResult<BootstrapStatus>> {
  return getJson("/api/v1/bootstrap", parseBootstrapStatus, options);
}
