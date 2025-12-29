/// <reference types="vite/client" />

interface UTXO {
  txid: string;
  vout: number;
  satoshis: number;
  scriptPk: string;
  addressType: number;
  inscriptions: unknown[];
  atomicals: unknown[];
}

interface Window {
  unisat?: {
    requestAccounts: () => Promise<string[]>;
    getAccounts: () => Promise<string[]>;
    getPublicKey: () => Promise<string>;
    signMessage: (message: string, type?: string) => Promise<string>;
    getNetwork: () => Promise<string>;
    switchNetwork: (network: string) => Promise<void>;
    sendBitcoin: (to: string, amount: number, options?: { feeRate?: number }) => Promise<string>;
    signPsbt: (psbtHex: string, options?: { autoFinalized?: boolean; toSignInputs?: { index: number; publicKey: string }[] }) => Promise<string>;
    getBalance: () => Promise<{ confirmed: number; unconfirmed: number; total: number }>;
    getBitcoinUtxos: () => Promise<UTXO[]>;
    pushPsbt: (psbtHex: string) => Promise<string>;
  };
}
