// Links to the block explorer (esplora UI).
import { chain } from './store.svelte'

/** Explorer base URL: configured on the server, or this host on port 5000. */
export const explorerBase = () => chain.explorerUrl || `${location.protocol}//${location.hostname}:5000`
export const txUrl = (txid: string) => `${explorerBase()}/tx/${txid}`
export const addressUrl = (address: string) => `${explorerBase()}/address/${address}`
