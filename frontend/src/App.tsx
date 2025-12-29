import { useState, useRef } from 'react';
import { Buffer } from 'buffer';
import * as bitcoin from 'bitcoinjs-lib';
import { BNS_API_URL } from './config';

// Polyfill Buffer for browser
if (typeof window !== 'undefined') {
  (window as unknown as { Buffer: typeof Buffer }).Buffer = Buffer;
}

// Pool address for listing transactions (testnet)
const POOL_ADDRESS = 'tb1qkry5g4xm7gstpjczhdwycgxsvdflhf4d0nt7z3';

// Bitcoin testnet network
const network = bitcoin.networks.testnet;

// Styles
const styles = {
  container: {
    padding: '20px',
  },
  header: {
    textAlign: 'center' as const,
    marginBottom: '40px',
  },
  title: {
    fontSize: '2rem',
    marginBottom: '10px',
  },
  subtitle: {
    color: '#888',
    fontSize: '0.9rem',
  },
  card: {
    background: 'rgba(255,255,255,0.1)',
    borderRadius: '12px',
    padding: '20px',
    marginBottom: '20px',
  },
  label: {
    color: '#888',
    fontSize: '0.85rem',
    marginBottom: '8px',
  },
  value: {
    wordBreak: 'break-all' as const,
    fontFamily: 'monospace',
    fontSize: '0.9rem',
  },
  button: {
    background: '#f7931a',
    color: '#fff',
    border: 'none',
    padding: '12px 24px',
    borderRadius: '8px',
    fontSize: '1rem',
    cursor: 'pointer',
    marginRight: '10px',
    marginBottom: '10px',
  },
  buttonSecondary: {
    background: '#333',
    color: '#fff',
    border: '1px solid #555',
    padding: '12px 24px',
    borderRadius: '8px',
    fontSize: '1rem',
    cursor: 'pointer',
    marginRight: '10px',
    marginBottom: '10px',
  },
  buttonDisabled: {
    background: '#555',
    cursor: 'not-allowed',
  },
  log: {
    background: '#000',
    borderRadius: '8px',
    padding: '15px',
    fontFamily: 'monospace',
    fontSize: '0.8rem',
    maxHeight: '400px',
    overflow: 'auto',
    whiteSpace: 'pre-wrap' as const,
  },
  error: {
    color: '#ff6b6b',
  },
  success: {
    color: '#51cf66',
  },
};

function App() {
  const [logs, setLogs] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [btcAddress, setBtcAddress] = useState<string | null>(null);
  const [bnsSession, setBnsSession] = useState<{
    session_id: string;
    btc_address: string;
    expires_at: string;
    is_new_user: boolean;
  } | null>(null);
  const logsRef = useRef<string[]>([]);

  const addLog = (message: string, type: 'info' | 'error' | 'success' = 'info') => {
    const timestamp = new Date().toLocaleTimeString();
    const prefix = type === 'error' ? '[ERROR]' : type === 'success' ? '[SUCCESS]' : '[INFO]';
    const logEntry = `${timestamp} ${prefix} ${message}`;
    logsRef.current = [...logsRef.current, logEntry];
    setLogs([...logsRef.current]);
    console.log(`${prefix} ${message}`);
  };

  const handleConnectWallet = async () => {
    try {
      setIsLoading(true);
      addLog('Connecting to UniSat wallet...');

      if (!window.unisat) {
        addLog('UniSat wallet not found. Please install UniSat extension.', 'error');
        return;
      }

      const accounts = await window.unisat.requestAccounts();
      if (accounts.length === 0) {
        addLog('No accounts returned from UniSat', 'error');
        return;
      }

      const address = accounts[0];
      setBtcAddress(address);
      addLog(`Connected: ${address}`, 'success');
    } catch (error) {
      addLog(`Error connecting wallet: ${error}`, 'error');
    } finally {
      setIsLoading(false);
    }
  };

  const handleLogin = async () => {
    try {
      setIsLoading(true);
      logsRef.current = [];
      setLogs([]);

      addLog('Starting BIP-322 login flow...');

      // Step 1: Connect wallet if not connected
      if (!btcAddress) {
        addLog('Step 1: Connecting wallet...');
        if (!window.unisat) {
          addLog('UniSat wallet not found. Please install UniSat extension.', 'error');
          return;
        }

        const accounts = await window.unisat.requestAccounts();
        if (accounts.length === 0) {
          addLog('No accounts returned', 'error');
          return;
        }
        const address = accounts[0];
        setBtcAddress(address);
        addLog(`Connected: ${address}`, 'success');
      }

      const address = btcAddress || (await window.unisat!.getAccounts())[0];

      // Step 2: Create and sign message
      // Message format: "Sign in to bns.zone at {timestamp} with nonce {nonce}"
      const timestamp = Math.floor(Date.now() / 1000); // Unix timestamp in seconds
      const nonce = crypto.randomUUID().replace(/-/g, '').substring(0, 16); // 16 char hex nonce
      const message = `Sign in to bns.zone at ${timestamp} with nonce ${nonce}`;
      addLog(`Step 2: Signing message: ${message}`);

      const signature = await window.unisat!.signMessage(message, 'bip322-simple');
      addLog(`Signature: ${signature.substring(0, 50)}...`, 'success');

      // Step 3: Send to BNS server
      addLog(`Step 3: Authenticating with BNS Server...`);
      addLog(`POST ${BNS_API_URL}/v1/auth/login`);

      const requestBody = {
        address,
        message,
        signature,
      };

      addLog(`Request: ${JSON.stringify(requestBody).substring(0, 150)}...`);

      const response = await fetch(`${BNS_API_URL}/v1/auth/login`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(requestBody),
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`API error: ${response.status} - ${errorText}`);
      }

      const session = await response.json();
      setBnsSession(session);

      addLog('=== LOGIN SUCCESS ===', 'success');
      addLog(JSON.stringify(session, null, 2), 'success');

      // Store session token
      localStorage.setItem('bns_session_id', session.session_id);
    } catch (error) {
      addLog(`Login error: ${error}`, 'error');
    } finally {
      setIsLoading(false);
    }
  };

  const handleGetMe = async () => {
    try {
      setIsLoading(true);
      const sessionToken = localStorage.getItem('bns_session_id');
      if (!sessionToken) {
        addLog('No session token found', 'error');
        return;
      }

      addLog(`GET ${BNS_API_URL}/v1/auth/me`);

      const response = await fetch(`${BNS_API_URL}/v1/auth/me`, {
        headers: {
          'Authorization': `Bearer ${sessionToken}`,
        },
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`API error: ${response.status} - ${errorText}`);
      }

      const data = await response.json();
      addLog('=== SESSION INFO ===', 'success');
      addLog(JSON.stringify(data, null, 2), 'success');
    } catch (error) {
      addLog(`Get me error: ${error}`, 'error');
    } finally {
      setIsLoading(false);
    }
  };

  const handleLogout = async () => {
    try {
      setIsLoading(true);
      const sessionToken = localStorage.getItem('bns_session_id');
      if (!sessionToken) {
        addLog('No session token found', 'error');
        return;
      }

      addLog(`POST ${BNS_API_URL}/v1/auth/logout`);

      const response = await fetch(`${BNS_API_URL}/v1/auth/logout`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${sessionToken}`,
        },
      });

      if (!response.ok && response.status !== 204) {
        const errorText = await response.text();
        throw new Error(`Logout error: ${response.status} - ${errorText}`);
      }

      addLog('Logged out successfully', 'success');
      setBnsSession(null);
      localStorage.removeItem('bns_session_id');
    } catch (error) {
      addLog(`Logout error: ${error}`, 'error');
    } finally {
      setIsLoading(false);
    }
  };

  const handleClear = () => {
    logsRef.current = [];
    setLogs([]);
    setBtcAddress(null);
    setBnsSession(null);
    setIsLoading(false);
    localStorage.removeItem('bns_session_id');
  };

  const handleListName = async () => {
    try {
      setIsLoading(true);
      addLog('Starting list name flow with PSBT...');

      // Ensure wallet is connected
      if (!btcAddress) {
        addLog('Please connect wallet first', 'error');
        return;
      }

      if (!window.unisat) {
        addLog('UniSat wallet not found', 'error');
        return;
      }

      // Test data
      const testName = 'test.btc';
      const testPriceSats = 100000; // 0.001 BTC
      const listingAmountSats = 1000; // Small amount to send to pool
      const feeRate = 2; // sat/vB

      addLog(`Listing name: ${testName} for ${testPriceSats} sats`);
      addLog(`Sending ${listingAmountSats} sats to pool address: ${POOL_ADDRESS}`);

      try {
        // Step 1: Get UTXOs from wallet
        addLog('Step 1: Getting UTXOs from wallet...');
        const utxos = await window.unisat.getBitcoinUtxos();
        addLog(`Found ${utxos.length} UTXOs`);

        if (utxos.length === 0) {
          addLog('No UTXOs available', 'error');
          return;
        }

        // Step 2: Get public key
        addLog('Step 2: Getting public key...');
        const publicKey = await window.unisat.getPublicKey();
        addLog(`Public key: ${publicKey.substring(0, 20)}...`);

        // Step 3: Build PSBT
        addLog('Step 3: Building PSBT...');
        const psbt = new bitcoin.Psbt({ network });

        // Calculate total input and select UTXOs
        const estimatedTxSize = 150; // Rough estimate for 1 input, 2 outputs
        const estimatedFee = estimatedTxSize * feeRate;
        const totalNeeded = listingAmountSats + estimatedFee;

        let totalInput = 0;
        const selectedUtxos: typeof utxos = [];

        for (const utxo of utxos) {
          if (totalInput >= totalNeeded) break;
          selectedUtxos.push(utxo);
          totalInput += utxo.satoshis;
        }

        if (totalInput < totalNeeded) {
          addLog(`Insufficient funds: have ${totalInput}, need ${totalNeeded}`, 'error');
          return;
        }

        addLog(`Selected ${selectedUtxos.length} UTXOs, total: ${totalInput} sats`);

        // Add inputs
        for (const utxo of selectedUtxos) {
          psbt.addInput({
            hash: utxo.txid,
            index: utxo.vout,
            witnessUtxo: {
              script: Buffer.from(utxo.scriptPk, 'hex'),
              value: BigInt(utxo.satoshis),
            },
          });
        }

        // Add output to pool address
        const poolOutput = bitcoin.address.toOutputScript(POOL_ADDRESS, network);
        psbt.addOutput({
          script: poolOutput,
          value: BigInt(listingAmountSats),
        });

        // Add change output back to sender
        const changeAmount = totalInput - listingAmountSats - estimatedFee;
        if (changeAmount > 546) { // Dust threshold
          const changeOutput = bitcoin.address.toOutputScript(btcAddress, network);
          psbt.addOutput({
            script: changeOutput,
            value: BigInt(changeAmount),
          });
        }

        // Convert PSBT to hex
        const psbtHex = psbt.toHex();
        addLog(`PSBT created: ${psbtHex.substring(0, 50)}...`);

        // Step 4: Sign PSBT with wallet
        addLog('Step 4: Signing PSBT with wallet...');
        const toSignInputs = selectedUtxos.map((_, index) => ({
          index,
          publicKey,
        }));

        const signedPsbtHex = await window.unisat.signPsbt(psbtHex, {
          autoFinalized: false, // Keep as PSBT, server will finalize
          toSignInputs,
        });
        addLog('PSBT signed successfully', 'success');
        addLog(`Signed PSBT hex length: ${signedPsbtHex.length}`);

        // Convert signed PSBT hex to base64 for server
        const signedPsbtBase64 = Buffer.from(signedPsbtHex, 'hex').toString('base64');
        addLog(`Signed PSBT base64 length: ${signedPsbtBase64.length}`);

        // Step 5: Send to server
        addLog('Step 5: Sending listing request to server...');
        const requestBody = {
          name: testName,
          priceSats: testPriceSats,
          sellerAddress: btcAddress,
          psbt: signedPsbtBase64,
        };

        addLog(`POST ${BNS_API_URL}/v1/listings`);

        const response = await fetch(`${BNS_API_URL}/v1/listings`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify(requestBody),
        });

        if (!response.ok) {
          const errorText = await response.text();
          addLog(`Server error: ${errorText}`, 'error');
        } else {
          const listing = await response.json();
          addLog('=== LISTING SUCCESS ===', 'success');
          addLog(JSON.stringify(listing, null, 2), 'success');
        }
      } catch (walletError) {
        addLog(`Error: ${walletError}`, 'error');
      }
    } catch (error) {
      addLog(`List name error: ${error}`, 'error');
    } finally {
      setIsLoading(false);
    }
  };

  const handleGetListings = async () => {
    try {
      setIsLoading(true);
      addLog(`GET ${BNS_API_URL}/v1/listings`);

      const response = await fetch(`${BNS_API_URL}/v1/listings`);

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`API error: ${response.status} - ${errorText}`);
      }

      const data = await response.json();
      addLog('=== LISTINGS ===', 'success');
      addLog(JSON.stringify(data, null, 2), 'success');
    } catch (error) {
      addLog(`Get listings error: ${error}`, 'error');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <h1 style={styles.title}>BNS BIP-322 Login</h1>
        <p style={styles.subtitle}>
          API: {BNS_API_URL}
        </p>
      </div>

      {/* Status Card */}
      <div style={styles.card}>
        <div style={styles.label}>Status</div>
        <div style={styles.value}>
          {isLoading ? 'Loading...' :
           bnsSession ? 'Authenticated' :
           btcAddress ? 'Wallet Connected' :
           'Not Connected'}
        </div>
      </div>

      {/* BTC Address Card */}
      <div style={styles.card}>
        <div style={styles.label}>BTC Address</div>
        <div style={styles.value}>
          {btcAddress || 'Not connected'}
        </div>
      </div>

      {/* BNS Session Card */}
      <div style={styles.card}>
        <div style={styles.label}>BNS Session</div>
        <div style={styles.value}>
          {bnsSession ? (
            <div style={{ fontSize: '0.8rem' }}>
              <div><strong>Session ID:</strong> {bnsSession.session_id.substring(0, 36)}...</div>
              <div><strong>BTC Address:</strong> {bnsSession.btc_address}</div>
              <div><strong>New User:</strong> {bnsSession.is_new_user ? 'Yes' : 'No'}</div>
              <div><strong>Expires:</strong> {new Date(bnsSession.expires_at).toLocaleString()}</div>
            </div>
          ) : 'Not authenticated'}
        </div>
      </div>

      {/* Action Buttons */}
      <div style={styles.card}>
        <div style={styles.label}>Actions</div>
        <div>
          <button
            style={{
              ...styles.button,
              ...(isLoading ? styles.buttonDisabled : {}),
            }}
            onClick={handleLogin}
            disabled={isLoading || !!bnsSession}
          >
            {isLoading ? 'Loading...' : 'Login with UniSat'}
          </button>

          <button
            style={styles.buttonSecondary}
            onClick={handleConnectWallet}
            disabled={isLoading || !!btcAddress}
          >
            Connect Wallet
          </button>

          <button
            style={styles.buttonSecondary}
            onClick={handleGetMe}
            disabled={isLoading || !bnsSession}
          >
            Get Me
          </button>

          <button
            style={styles.buttonSecondary}
            onClick={handleLogout}
            disabled={isLoading || !bnsSession}
          >
            Logout
          </button>

          <button
            style={styles.buttonSecondary}
            onClick={handleClear}
          >
            Clear
          </button>
        </div>

        <div style={{ marginTop: '20px', borderTop: '1px solid #333', paddingTop: '20px' }}>
          <div style={styles.label}>Listing Actions</div>
          <button
            style={{
              ...styles.button,
              background: '#6c5ce7',
              ...(isLoading ? styles.buttonDisabled : {}),
            }}
            onClick={handleListName}
            disabled={isLoading || !btcAddress}
          >
            List Test Name
          </button>

          <button
            style={styles.buttonSecondary}
            onClick={handleGetListings}
            disabled={isLoading}
          >
            Get Listings
          </button>
        </div>
      </div>

      {/* Log Output */}
      <div style={styles.card}>
        <div style={styles.label}>Log Output</div>
        <div style={styles.log}>
          {logs.length === 0 ? 'Click "Login with UniSat" to start.' :
            logs.map((log, i) => (
              <div
                key={i}
                style={
                  log.includes('[ERROR]') ? styles.error :
                  log.includes('[SUCCESS]') ? styles.success :
                  {}
                }
              >
                {log}
              </div>
            ))
          }
        </div>
      </div>
    </div>
  );
}

export default App;
