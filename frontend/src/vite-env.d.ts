/// <reference types="vite/client" />

interface Window {
  unisat?: {
    requestAccounts: () => Promise<string[]>;
    getAccounts: () => Promise<string[]>;
    getPublicKey: () => Promise<string>;
    signMessage: (message: string, type?: string) => Promise<string>;
    getNetwork: () => Promise<string>;
    switchNetwork: (network: string) => Promise<void>;
  };
}
