// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * @file Pure zero-dependency Open Sound Control (OSC 1.0) binary encoder and decoder.
 * Usable in Node.js, Electron, and browser environments supporting Buffer / Uint8Array.
 */

import { Buffer } from 'node:buffer';

/**
 * Calculates 4-byte aligned boundary length for null-terminated strings.
 * @param {number} strByteLen - Byte length of string without null terminator.
 * @returns {number} Padded total byte length (always a multiple of 4).
 */
export function paddedStringLen(strByteLen) {
  return (strByteLen + 4) & ~3;
}

/**
 * Calculates 4-byte aligned boundary length for arbitrary binary blobs.
 * @param {number} blobLen - Byte length of raw blob data.
 * @returns {number} Padded total byte length (always a multiple of 4).
 */
export function paddedBlobLen(blobLen) {
  return (blobLen + 3) & ~3;
}

/**
 * Helper to construct an explicit 32-bit float argument.
 * @param {number} value
 * @returns {{type: string, value: number}}
 */
export function oscFloat(value) {
  return { type: 'f', value: Number(value) };
}

/**
 * Helper to construct an explicit 32-bit integer argument.
 * @param {number} value
 * @returns {{type: string, value: number}}
 */
export function oscInt(value) {
  return { type: 'i', value: Math.trunc(value) };
}

/**
 * Helper to construct an explicit string argument.
 * @param {string} value
 * @returns {{type: string, value: string}}
 */
export function oscString(value) {
  return { type: 's', value: String(value) };
}

/**
 * Helper to construct an explicit binary blob argument.
 * @param {Buffer|Uint8Array|number[]} value
 * @returns {{type: string, value: Buffer}}
 */
export function oscBlob(value) {
  return { type: 'b', value: Buffer.isBuffer(value) ? value : Buffer.from(value) };
}

/**
 * Helper to construct an explicit boolean argument.
 * @param {boolean} value
 * @returns {{type: string, value: boolean}}
 */
export function oscBool(value) {
  return { type: Boolean(value) ? 'T' : 'F', value: Boolean(value) };
}

/**
 * Determines OSC type tag and normalized value for a given argument.
 * @param {*} arg
 * @returns {{type: string, value: *}}
 */
export function normalizeArg(arg) {
  if (arg === null || arg === undefined) {
    return { type: 'N', value: null };
  }
  if (typeof arg === 'boolean') {
    return { type: arg ? 'T' : 'F', value: arg };
  }
  if (typeof arg === 'string') {
    return { type: 's', value: arg };
  }
  if (typeof arg === 'bigint') {
    return { type: 'h', value: arg };
  }
  if (Buffer.isBuffer(arg) || arg instanceof Uint8Array) {
    return { type: 'b', value: Buffer.isBuffer(arg) ? arg : Buffer.from(arg.buffer, arg.byteOffset, arg.byteLength) };
  }
  if (typeof arg === 'number') {
    if (Number.isInteger(arg) && arg >= -2147483648 && arg <= 2147483647) {
      return { type: 'i', value: arg };
    }
    return { type: 'f', value: arg };
  }
  if (typeof arg === 'object' && typeof arg.type === 'string') {
    return arg;
  }
  return { type: 's', value: String(arg) };
}

/**
 * Encodes an OSC Message into a Node.js Buffer.
 *
 * @param {string} address - OSC address pattern (must start with '/').
 * @param {Array<*>} [args=[]] - List of arguments (primitives or explicit {type, value} objects).
 * @returns {Buffer} Binary OSC packet.
 */
export function encodeMessage(address, args = []) {
  if (typeof address !== 'string' || !address.startsWith('/')) {
    throw new TypeError(`OSC address pattern must be a string starting with '/', got: ${address}`);
  }

  const addrBuf = Buffer.from(address, 'utf8');
  const addrLen = paddedStringLen(addrBuf.length);

  const normalized = args.map(normalizeArg);
  const typeTagStr = ',' + normalized.map((a) => a.type).join('');
  const typeTagBuf = Buffer.from(typeTagStr, 'ascii');
  const typeTagLen = paddedStringLen(typeTagBuf.length);

  let payloadLen = 0;
  for (const item of normalized) {
    switch (item.type) {
      case 'i':
      case 'f':
        payloadLen += 4;
        break;
      case 'd':
      case 'h':
      case 't':
        payloadLen += 8;
        break;
      case 's': {
        const strBytes = Buffer.byteLength(String(item.value), 'utf8');
        payloadLen += paddedStringLen(strBytes);
        break;
      }
      case 'b': {
        const blobBytes = item.value.length;
        payloadLen += 4 + paddedBlobLen(blobBytes);
        break;
      }
      case 'T':
      case 'F':
      case 'N':
      case 'I':
        // Zero payload bytes
        break;
      default:
        throw new Error(`Unsupported OSC type tag: ${item.type}`);
    }
  }

  const totalLen = addrLen + typeTagLen + payloadLen;
  const buffer = Buffer.alloc(totalLen, 0);

  // 1. Write Address
  addrBuf.copy(buffer, 0);
  let offset = addrLen;

  // 2. Write Type Tag String
  typeTagBuf.copy(buffer, offset);
  offset += typeTagLen;

  // 3. Write Argument Payloads
  for (const item of normalized) {
    switch (item.type) {
      case 'i':
        buffer.writeInt32BE(Number(item.value), offset);
        offset += 4;
        break;
      case 'f':
        buffer.writeFloatBE(Number(item.value), offset);
        offset += 4;
        break;
      case 'd':
        buffer.writeDoubleBE(Number(item.value), offset);
        offset += 8;
        break;
      case 'h':
        buffer.writeBigInt64BE(BigInt(item.value), offset);
        offset += 8;
        break;
      case 's': {
        const s = String(item.value);
        const sLen = buffer.write(s, offset, 'utf8');
        offset += paddedStringLen(sLen);
        break;
      }
      case 'b': {
        const blob = item.value;
        buffer.writeInt32BE(blob.length, offset);
        offset += 4;
        blob.copy(buffer, offset);
        offset += paddedBlobLen(blob.length);
        break;
      }
      case 'T':
      case 'F':
      case 'N':
      case 'I':
        // No payload bytes
        break;
    }
  }

  return buffer;
}

/**
 * Decodes a null-terminated OSC string from a buffer.
 * @param {Buffer} buffer
 * @param {number} offset
 * @returns {{value: string, nextOffset: number}}
 */
function readOscString(buffer, offset) {
  let nullIdx = offset;
  while (nullIdx < buffer.length && buffer[nullIdx] !== 0) {
    nullIdx++;
  }
  if (nullIdx >= buffer.length) {
    throw new RangeError(`Unterminated OSC string at offset ${offset}`);
  }
  const value = buffer.toString('utf8', offset, nullIdx);
  const nextOffset = (nullIdx + 4) & ~3;
  return { value, nextOffset };
}

/**
 * Decodes an OSC Message from a buffer slice.
 *
 * @param {Buffer} buffer - OSC packet buffer.
 * @param {number} [offset=0] - Starting byte offset.
 * @returns {{isBundle: boolean, address: string, types: string, args: Array<*>}}
 */
export function decodeMessage(buffer, offset = 0) {
  const { value: address, nextOffset: afterAddr } = readOscString(buffer, offset);
  if (!address.startsWith('/')) {
    throw new Error(`Invalid OSC address pattern: ${address}`);
  }

  if (afterAddr >= buffer.length) {
    return { isBundle: false, address, types: '', args: [] };
  }

  const { value: typeTags, nextOffset: afterTags } = readOscString(buffer, afterAddr);
  if (!typeTags.startsWith(',')) {
    // Message with no type tags (allowed in some OSC dialects, treated as no args)
    return { isBundle: false, address, types: '', args: [] };
  }

  let curOffset = afterTags;
  const args = [];
  const tagChars = typeTags.slice(1);

  for (let i = 0; i < tagChars.length; i++) {
    const tag = tagChars[i];
    switch (tag) {
      case 'i': {
        const val = buffer.readInt32BE(curOffset);
        curOffset += 4;
        args.push(val);
        break;
      }
      case 'f': {
        const val = buffer.readFloatBE(curOffset);
        curOffset += 4;
        args.push(val);
        break;
      }
      case 's': {
        const { value: strVal, nextOffset: next } = readOscString(buffer, curOffset);
        curOffset = next;
        args.push(strVal);
        break;
      }
      case 'b': {
        const blobLen = buffer.readInt32BE(curOffset);
        curOffset += 4;
        const blobData = buffer.subarray(curOffset, curOffset + blobLen);
        curOffset += paddedBlobLen(blobLen);
        args.push(Buffer.from(blobData));
        break;
      }
      case 'T':
        args.push(true);
        break;
      case 'F':
        args.push(false);
        break;
      case 'N':
        args.push(null);
        break;
      case 'I':
        args.push(Infinity);
        break;
      case 'd': {
        const val = buffer.readDoubleBE(curOffset);
        curOffset += 8;
        args.push(val);
        break;
      }
      case 'h': {
        const val = buffer.readBigInt64BE(curOffset);
        curOffset += 8;
        args.push(val);
        break;
      }
      case 't': {
        const seconds = buffer.readUInt32BE(curOffset);
        const fraction = buffer.readUInt32BE(curOffset + 4);
        curOffset += 8;
        args.push({ seconds, fraction });
        break;
      }
      default:
        throw new Error(`Unsupported type tag '${tag}' at argument index ${i}`);
    }
  }

  return {
    isBundle: false,
    address,
    types: tagChars,
    args,
  };
}

/**
 * Encodes an OSC Bundle into a Node.js Buffer.
 *
 * @param {bigint|number|{seconds: number, fraction: number}} timetag - 64-bit NTP timetag (1n = immediate).
 * @param {Array<Buffer>} elements - Encoded OSC message or bundle buffers.
 * @returns {Buffer} Binary OSC bundle.
 */
export function encodeBundle(timetag = 1n, elements = []) {
  const header = Buffer.from('#bundle\0', 'ascii');
  const timetagBuf = Buffer.alloc(8, 0);

  if (typeof timetag === 'bigint') {
    timetagBuf.writeBigUInt64BE(timetag, 0);
  } else if (typeof timetag === 'number') {
    timetagBuf.writeBigUInt64BE(BigInt(timetag), 0);
  } else if (typeof timetag === 'object' && timetag !== null) {
    timetagBuf.writeUInt32BE(timetag.seconds || 0, 0);
    timetagBuf.writeUInt32BE(timetag.fraction || 0, 4);
  }

  let totalElementsLen = 0;
  for (const elem of elements) {
    totalElementsLen += 4 + elem.length;
  }

  const bundleBuf = Buffer.alloc(16 + totalElementsLen, 0);
  header.copy(bundleBuf, 0);
  timetagBuf.copy(bundleBuf, 8);

  let offset = 16;
  for (const elem of elements) {
    bundleBuf.writeInt32BE(elem.length, offset);
    offset += 4;
    elem.copy(bundleBuf, offset);
    offset += elem.length;
  }

  return bundleBuf;
}

/**
 * Decodes an OSC Bundle from a buffer.
 *
 * @param {Buffer} buffer
 * @param {number} [offset=0]
 * @returns {{isBundle: boolean, timetag: bigint, elements: Array<*>}}
 */
export function decodeBundle(buffer, offset = 0) {
  if (buffer.subarray(offset, offset + 8).toString('ascii') !== '#bundle\0') {
    throw new Error('Invalid OSC bundle header');
  }

  const timetag = buffer.readBigUInt64BE(offset + 8);
  let curOffset = offset + 16;
  const elements = [];

  while (curOffset < buffer.length) {
    const elemLen = buffer.readInt32BE(curOffset);
    curOffset += 4;
    const elemSlice = buffer.subarray(curOffset, curOffset + elemLen);
    curOffset += elemLen;
    elements.push(decodePacket(elemSlice));
  }

  return {
    isBundle: true,
    timetag,
    elements,
  };
}

/**
 * Universal packet decoder for either an OSC Message or OSC Bundle.
 *
 * @param {Buffer} buffer - Incoming UDP packet bytes.
 * @returns {{isBundle: boolean, address?: string, types?: string, args?: Array<*>, timetag?: bigint, elements?: Array<*>}}
 */
export function decodePacket(buffer) {
  if (!buffer || buffer.length === 0) {
    throw new Error('Cannot decode empty OSC buffer');
  }

  if (buffer.length >= 8 && buffer.subarray(0, 8).toString('ascii') === '#bundle\0') {
    return decodeBundle(buffer, 0);
  }

  return decodeMessage(buffer, 0);
}
