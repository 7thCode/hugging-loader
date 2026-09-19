import { promises as fs } from 'fs'
import type { FileHandle } from 'fs/promises'
import type {
  GgufHeaderResponse,
  GgufMetadataEntry,
  GgufScalar,
  GgufTensorInfo
} from '../../shared/ipc-types'

// Layout and enum values follow gguf-py (llama.cpp/gguf-py/gguf/constants.py).
const GGUF_MAGIC = 'GGUF'
const DEFAULT_ALIGNMENT = 32
const READ_WINDOW = 1024 * 1024
const ARRAY_PREVIEW_LENGTH = 16
// Guards against runaway loops on corrupt files; real models are far below these.
const MAX_METADATA_COUNT = 1_000_000
const MAX_TENSOR_COUNT = 1_000_000
const MAX_TENSOR_DIMS = 8

const VALUE_TYPE = {
  UINT8: 0,
  INT8: 1,
  UINT16: 2,
  INT16: 3,
  UINT32: 4,
  INT32: 5,
  FLOAT32: 6,
  BOOL: 7,
  STRING: 8,
  ARRAY: 9,
  UINT64: 10,
  INT64: 11,
  FLOAT64: 12
} as const

const VALUE_TYPE_NAMES: Record<number, string> = {
  0: 'uint8',
  1: 'int8',
  2: 'uint16',
  3: 'int16',
  4: 'uint32',
  5: 'int32',
  6: 'float32',
  7: 'bool',
  8: 'string',
  9: 'array',
  10: 'uint64',
  11: 'int64',
  12: 'float64'
}

const FIXED_VALUE_SIZES: Record<number, number> = {
  0: 1,
  1: 1,
  2: 2,
  3: 2,
  4: 4,
  5: 4,
  6: 4,
  7: 1,
  10: 8,
  11: 8,
  12: 8
}

const GGML_TYPE_NAMES: Record<number, string> = {
  0: 'F32',
  1: 'F16',
  2: 'Q4_0',
  3: 'Q4_1',
  6: 'Q5_0',
  7: 'Q5_1',
  8: 'Q8_0',
  9: 'Q8_1',
  10: 'Q2_K',
  11: 'Q3_K',
  12: 'Q4_K',
  13: 'Q5_K',
  14: 'Q6_K',
  15: 'Q8_K',
  16: 'IQ2_XXS',
  17: 'IQ2_XS',
  18: 'IQ3_XXS',
  19: 'IQ1_S',
  20: 'IQ4_NL',
  21: 'IQ3_S',
  22: 'IQ2_S',
  23: 'IQ4_XS',
  24: 'I8',
  25: 'I16',
  26: 'I32',
  27: 'I64',
  28: 'F64',
  29: 'IQ1_M',
  30: 'BF16',
  34: 'TQ1_0',
  35: 'TQ2_0',
  39: 'MXFP4',
  40: 'NVFP4',
  41: 'Q1_0',
  42: 'Q2_0'
}

// llama_ftype: the value of `general.file_type`.
const FILE_TYPE_NAMES: Record<number, string> = {
  0: 'ALL_F32',
  1: 'MOSTLY_F16',
  2: 'MOSTLY_Q4_0',
  3: 'MOSTLY_Q4_1',
  7: 'MOSTLY_Q8_0',
  8: 'MOSTLY_Q5_0',
  9: 'MOSTLY_Q5_1',
  10: 'MOSTLY_Q2_K',
  11: 'MOSTLY_Q3_K_S',
  12: 'MOSTLY_Q3_K_M',
  13: 'MOSTLY_Q3_K_L',
  14: 'MOSTLY_Q4_K_S',
  15: 'MOSTLY_Q4_K_M',
  16: 'MOSTLY_Q5_K_S',
  17: 'MOSTLY_Q5_K_M',
  18: 'MOSTLY_Q6_K',
  19: 'MOSTLY_IQ2_XXS',
  20: 'MOSTLY_IQ2_XS',
  21: 'MOSTLY_Q2_K_S',
  22: 'MOSTLY_IQ3_XS',
  23: 'MOSTLY_IQ3_XXS',
  24: 'MOSTLY_IQ1_S',
  25: 'MOSTLY_IQ4_NL',
  26: 'MOSTLY_IQ3_S',
  27: 'MOSTLY_IQ3_M',
  28: 'MOSTLY_IQ2_S',
  29: 'MOSTLY_IQ2_M',
  30: 'MOSTLY_IQ4_XS',
  31: 'MOSTLY_IQ1_M',
  32: 'MOSTLY_BF16',
  36: 'MOSTLY_TQ1_0',
  37: 'MOSTLY_TQ2_0',
  38: 'MOSTLY_MXFP4_MOE',
  39: 'MOSTLY_NVFP4',
  40: 'MOSTLY_Q1_0',
  41: 'MOSTLY_Q2_0',
  1024: 'GUESSED'
}

// Sequential reader over a file that keeps a sliding read window, so a header with a
// multi-megabyte tokenizer array is parsed without loading the whole file into memory.
class HeaderReader {
  private window: Buffer = Buffer.alloc(0)
  private windowStart = 0
  private readonly handle: FileHandle
  readonly fileSize: number
  position = 0

  constructor(handle: FileHandle, fileSize: number) {
    this.handle = handle
    this.fileSize = fileSize
  }

  async read(length: number): Promise<Buffer> {
    if (length < 0 || this.position + length > this.fileSize) {
      throw new Error(
        'GGUFヘッダーの途中でファイルが終了しました（破損または途中までのファイルの可能性）'
      )
    }
    const end = this.position + length
    const inWindow =
      this.position >= this.windowStart && end <= this.windowStart + this.window.length
    if (!inWindow) {
      const size = Math.min(Math.max(length, READ_WINDOW), this.fileSize - this.position)
      const buffer = Buffer.alloc(size)
      const { bytesRead } = await this.handle.read(buffer, 0, size, this.position)
      if (bytesRead < length) {
        throw new Error('GGUFヘッダーの読み込みに失敗しました')
      }
      this.window = buffer.subarray(0, bytesRead)
      this.windowStart = this.position
    }
    const out = this.window.subarray(this.position - this.windowStart, end - this.windowStart)
    this.position = end
    return out
  }

  skip(length: number): void {
    if (length < 0 || this.position + length > this.fileSize) {
      throw new Error(
        'GGUFヘッダーの途中でファイルが終了しました（破損または途中までのファイルの可能性）'
      )
    }
    this.position += length
  }

  async u32(): Promise<number> {
    return (await this.read(4)).readUInt32LE(0)
  }

  async u64(): Promise<number | string> {
    return bigIntToNumber((await this.read(8)).readBigUInt64LE(0))
  }

  async string(): Promise<string> {
    const length = Number((await this.read(8)).readBigUInt64LE(0))
    return (await this.read(length)).toString('utf-8')
  }
}

// 64-bit integers beyond 2^53 lose precision as a JS number, so they're kept as decimal strings.
function bigIntToNumber(value: bigint): number | string {
  return value <= BigInt(Number.MAX_SAFE_INTEGER) && value >= BigInt(Number.MIN_SAFE_INTEGER)
    ? Number(value)
    : value.toString()
}

async function readScalar(reader: HeaderReader, type: number): Promise<GgufScalar> {
  switch (type) {
    case VALUE_TYPE.UINT8:
      return (await reader.read(1)).readUInt8(0)
    case VALUE_TYPE.INT8:
      return (await reader.read(1)).readInt8(0)
    case VALUE_TYPE.UINT16:
      return (await reader.read(2)).readUInt16LE(0)
    case VALUE_TYPE.INT16:
      return (await reader.read(2)).readInt16LE(0)
    case VALUE_TYPE.UINT32:
      return (await reader.read(4)).readUInt32LE(0)
    case VALUE_TYPE.INT32:
      return (await reader.read(4)).readInt32LE(0)
    case VALUE_TYPE.FLOAT32:
      return (await reader.read(4)).readFloatLE(0)
    case VALUE_TYPE.BOOL:
      return (await reader.read(1)).readUInt8(0) !== 0
    case VALUE_TYPE.STRING:
      return reader.string()
    case VALUE_TYPE.UINT64:
      return bigIntToNumber((await reader.read(8)).readBigUInt64LE(0))
    case VALUE_TYPE.INT64:
      return bigIntToNumber((await reader.read(8)).readBigInt64LE(0))
    case VALUE_TYPE.FLOAT64:
      return (await reader.read(8)).readDoubleLE(0)
    default:
      throw new Error(`未対応のGGUF値タイプです: ${type}`)
  }
}

async function readMetadataEntry(reader: HeaderReader): Promise<GgufMetadataEntry> {
  const key = await reader.string()
  const type = await reader.u32()

  if (type !== VALUE_TYPE.ARRAY) {
    const value = await readScalar(reader, type)
    return { key, type: VALUE_TYPE_NAMES[type] ?? String(type), value, array: null }
  }

  const elementType = await reader.u32()
  const length = Number(await reader.u64())
  const elementTypeName = VALUE_TYPE_NAMES[elementType] ?? String(elementType)
  const preview: GgufScalar[] = []

  const fixedSize = FIXED_VALUE_SIZES[elementType]
  if (fixedSize !== undefined) {
    const previewCount = Math.min(length, ARRAY_PREVIEW_LENGTH)
    for (let i = 0; i < previewCount; i++) {
      preview.push(await readScalar(reader, elementType))
    }
    reader.skip((length - previewCount) * fixedSize)
  } else if (elementType === VALUE_TYPE.STRING) {
    // Strings are length-prefixed, so every element has to be stepped over to reach the next key.
    for (let i = 0; i < length; i++) {
      const size = Number((await reader.read(8)).readBigUInt64LE(0))
      if (i < ARRAY_PREVIEW_LENGTH) {
        preview.push((await reader.read(size)).toString('utf-8'))
      } else {
        reader.skip(size)
      }
    }
  } else {
    throw new Error(`未対応の配列要素タイプです: ${elementType}`)
  }

  return {
    key,
    type: `array<${elementTypeName}>`,
    value: null,
    array: { elementType: elementTypeName, length, preview }
  }
}

async function readTensorInfo(reader: HeaderReader): Promise<GgufTensorInfo> {
  const name = await reader.string()
  const dimCount = await reader.u32()
  if (dimCount > MAX_TENSOR_DIMS) {
    throw new Error(`テンソルの次元数が不正です: ${dimCount}`)
  }
  const dims: number[] = []
  for (let i = 0; i < dimCount; i++) {
    dims.push(Number(await reader.u64()))
  }
  const ggmlType = await reader.u32()
  const offset = Number(await reader.u64())
  return { name, dims, type: GGML_TYPE_NAMES[ggmlType] ?? `type ${ggmlType}`, offset }
}

export async function readGgufHeader(filePath: string): Promise<GgufHeaderResponse> {
  const handle = await fs.open(filePath, 'r')
  try {
    const { size: fileSize } = await handle.stat()
    const reader = new HeaderReader(handle, fileSize)

    const magic = (await reader.read(4)).toString('ascii')
    if (magic !== GGUF_MAGIC) {
      throw new Error('GGUFファイルではありません（先頭のマジックバイトが "GGUF" と一致しません）')
    }

    const version = await reader.u32()
    if ((version & 0xffff) === 0) {
      throw new Error('ビッグエンディアンのGGUFファイルには対応していません')
    }
    if (version !== 2 && version !== 3) {
      throw new Error(`未対応のGGUFバージョンです: ${version}（対応: 2, 3）`)
    }

    const tensorCount = Number(await reader.u64())
    const metadataCount = Number(await reader.u64())
    if (tensorCount > MAX_TENSOR_COUNT || metadataCount > MAX_METADATA_COUNT) {
      throw new Error('GGUFヘッダーの件数が不正です（ファイルが破損している可能性）')
    }

    const metadata: GgufMetadataEntry[] = []
    for (let i = 0; i < metadataCount; i++) {
      metadata.push(await readMetadataEntry(reader))
    }

    const tensors: GgufTensorInfo[] = []
    for (let i = 0; i < tensorCount; i++) {
      tensors.push(await readTensorInfo(reader))
    }

    const headerSize = reader.position
    const alignmentEntry = metadata.find((entry) => entry.key === 'general.alignment')
    const alignment =
      typeof alignmentEntry?.value === 'number' && alignmentEntry.value > 0
        ? alignmentEntry.value
        : DEFAULT_ALIGNMENT
    const dataOffset = Math.ceil(headerSize / alignment) * alignment

    const fileTypeEntry = metadata.find((entry) => entry.key === 'general.file_type')
    const fileTypeName =
      typeof fileTypeEntry?.value === 'number'
        ? (FILE_TYPE_NAMES[fileTypeEntry.value] ?? null)
        : null

    return {
      fileSize,
      version,
      tensorCount,
      metadataCount,
      headerSize,
      alignment,
      dataOffset,
      fileTypeName,
      metadata,
      tensors
    }
  } finally {
    await handle.close()
  }
}
