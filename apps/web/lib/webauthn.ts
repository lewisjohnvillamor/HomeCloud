/**
 * Browser side of the WebAuthn ceremonies.
 *
 * The server speaks base64url JSON; the browser API speaks
 * `ArrayBuffer`. These helpers are the whole translation, kept in one
 * place because getting a single field wrong fails in ways that are hard
 * to read from a browser error.
 */

/** Whether this browser can do passkeys at all. */
export function isPasskeySupported(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.PublicKeyCredential === "function" &&
    typeof navigator.credentials?.create === "function"
  );
}

function fromBase64Url(value: string): ArrayBuffer {
  const padded = value.replace(/-/g, "+").replace(/_/g, "/");
  const binary = atob(padded.padEnd(padded.length + ((4 - (padded.length % 4)) % 4), "="));
  const bytes = new Uint8Array(binary.length);

  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }

  return bytes.buffer;
}

function toBase64Url(value: ArrayBuffer): string {
  const bytes = new Uint8Array(value);
  let binary = "";

  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }

  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

type Descriptor = { id: string; type: string; transports?: string[] };

function toDescriptors(descriptors: Descriptor[] | undefined): PublicKeyCredentialDescriptor[] {
  return (descriptors ?? []).map((descriptor) => ({
    id: fromBase64Url(descriptor.id),
    type: "public-key",
    transports: descriptor.transports as AuthenticatorTransport[] | undefined,
  }));
}

/** Runs the registration ceremony and returns what the server expects back. */
export async function createPasskey(options: unknown): Promise<unknown> {
  const publicKey = (options as { publicKey: Record<string, never> }).publicKey as unknown as {
    challenge: string;
    user: { id: string; name: string; displayName: string };
    excludeCredentials?: Descriptor[];
  } & PublicKeyCredentialCreationOptions;

  const credential = (await navigator.credentials.create({
    publicKey: {
      ...publicKey,
      challenge: fromBase64Url(publicKey.challenge),
      user: { ...publicKey.user, id: fromBase64Url(publicKey.user.id) },
      excludeCredentials: toDescriptors(publicKey.excludeCredentials),
    },
  })) as PublicKeyCredential | null;

  if (!credential) {
    throw new Error("The authenticator did not return a credential.");
  }

  const response = credential.response as AuthenticatorAttestationResponse;

  return {
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      attestationObject: toBase64Url(response.attestationObject),
      clientDataJSON: toBase64Url(response.clientDataJSON),
    },
    extensions: {},
  };
}

/** Runs the sign-in ceremony and returns what the server expects back. */
export async function authenticateWithPasskey(options: unknown): Promise<unknown> {
  const publicKey = (options as { publicKey: Record<string, never> }).publicKey as unknown as {
    challenge: string;
    allowCredentials?: Descriptor[];
  } & PublicKeyCredentialRequestOptions;

  const credential = (await navigator.credentials.get({
    publicKey: {
      ...publicKey,
      challenge: fromBase64Url(publicKey.challenge),
      allowCredentials: toDescriptors(publicKey.allowCredentials),
    },
  })) as PublicKeyCredential | null;

  if (!credential) {
    throw new Error("The authenticator did not return a credential.");
  }

  const response = credential.response as AuthenticatorAssertionResponse;

  return {
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      authenticatorData: toBase64Url(response.authenticatorData),
      clientDataJSON: toBase64Url(response.clientDataJSON),
      signature: toBase64Url(response.signature),
      userHandle: response.userHandle ? toBase64Url(response.userHandle) : null,
    },
    extensions: {},
  };
}
