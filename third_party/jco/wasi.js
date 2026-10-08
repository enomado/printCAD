var __defProp = Object.defineProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};

// node_modules/@bytecodealliance/preview2-shim/dist/browser/cli.js
var cli_exports = {};
__export(cli_exports, {
  _setArgs: () => _setArgs,
  _setCwd: () => _setCwd,
  _setEnv: () => _setEnv,
  _setStderr: () => _setStderr,
  _setStdin: () => _setStdin,
  _setStdout: () => _setStdout,
  createCli: () => createCli,
  environment: () => environment,
  exit: () => exit,
  stderr: () => stderr,
  stdin: () => stdin,
  stdout: () => stdout,
  terminalInput: () => terminalInput,
  terminalOutput: () => terminalOutput,
  terminalStderr: () => terminalStderr,
  terminalStdin: () => terminalStdin,
  terminalStdout: () => terminalStdout
});

// node_modules/@bytecodealliance/preview2-shim/dist/browser/io.js
var io_exports = {};
__export(io_exports, {
  error: () => error,
  inputStreamCreate: () => inputStreamCreate,
  ioErrorCreate: () => ioErrorCreate,
  outputStreamCreate: () => outputStreamCreate,
  poll: () => poll,
  pollableCreate: () => pollableCreate,
  streams: () => streams
});

// node_modules/@bytecodealliance/preview2-shim/dist/browser/common.js
var MAX_U64 = (1n << 64n) - 1n;
function checkedU64(value, name) {
  if (typeof value !== "bigint" || value < 0n || value > MAX_U64) {
    throw new TypeError(`${name} must be a valid u64`);
  }
  return value;
}
function checkedU64AsNumber(value, name) {
  checkedU64(value, name);
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new RangeError(`${name} exceeds JavaScript's safe integer range`);
  }
  return Number(value);
}

// node_modules/@bytecodealliance/preview2-shim/dist/browser/io.js
var id = 0;
var symbolDispose = Symbol.dispose || Symbol.for("dispose");
var checkedLength = (len, name = "length") => checkedU64AsNumber(len, name);
function closed() {
  throw { tag: "closed" };
}
var IoError = class extends Error {
  toDebugString() {
    return this.message;
  }
};
var ioErrorCreate = (message) => new IoError(message);
var InputStream = class _InputStream {
  id;
  handler;
  #open = true;
  #children = /* @__PURE__ */ new Set();
  static _create(handler) {
    const stream = new _InputStream();
    if (!handler) {
      console.trace("no handler");
    }
    stream.id = ++id;
    stream.handler = handler;
    return stream;
  }
  read(len) {
    checkedLength(len);
    if (!this.#open) {
      closed();
    }
    if (this.handler.read) {
      return this.handler.read.call(this, len);
    }
    return this.handler.blockingRead.call(this, len);
  }
  blockingRead(len) {
    checkedLength(len);
    if (!this.#open) {
      closed();
    }
    return this.handler.blockingRead.call(this, len);
  }
  skip(len) {
    checkedLength(len);
    if (!this.#open) {
      closed();
    }
    if (this.handler.skip) {
      return this.handler.skip.call(this, len);
    }
    if (this.handler.read) {
      const bytes = this.handler.read.call(this, len);
      return BigInt(bytes.byteLength);
    }
    return this.blockingSkip.call(this, len);
  }
  blockingSkip(len) {
    checkedLength(len);
    if (!this.#open) {
      closed();
    }
    if (this.handler.blockingSkip) {
      return this.handler.blockingSkip.call(this, len);
    }
    const bytes = this.handler.blockingRead.call(this, len);
    return BigInt(bytes.byteLength);
  }
  subscribe() {
    if (!this.#open) {
      return pollableCreate();
    }
    const pollable = this.handler.subscribe ? this.handler.subscribe.call(this) : pollableCreate();
    if (pollable instanceof Pollable) {
      this.#children.add(pollable);
      pollable._onDispose(() => this.#children.delete(pollable));
    }
    return pollable;
  }
  [symbolDispose]() {
    if (!this.#open) {
      return;
    }
    this.#open = false;
    for (const child of this.#children) {
      child._invalidate();
    }
    this.#children.clear();
    if (this.handler.drop) {
      this.handler.drop.call(this);
    }
  }
};
var inputStreamCreate = InputStream._create;
delete InputStream._create;
var OutputStream = class _OutputStream {
  id;
  open;
  handler;
  #permit = 0n;
  #children = /* @__PURE__ */ new Set();
  static _create(handler) {
    const stream = new _OutputStream();
    if (!handler) {
      console.trace("no handler");
    }
    stream.id = ++id;
    stream.open = true;
    stream.handler = handler;
    return stream;
  }
  checkWrite() {
    if (!this.open) {
      closed();
    }
    if (this.handler.checkWrite) {
      const permit = this.handler.checkWrite.call(this);
      checkedLength(permit, "write permit");
      this.#permit = permit;
      return permit;
    }
    this.#permit = 1000000n;
    return this.#permit;
  }
  write(buf) {
    if (!this.open) {
      closed();
    }
    if (BigInt(buf.byteLength) > this.#permit) {
      throw new Error("write exceeds the permit returned by checkWrite");
    }
    this.#permit -= BigInt(buf.byteLength);
    this.handler.write.call(this, buf);
  }
  blockingWriteAndFlush(buf) {
    if (!this.open) {
      closed();
    }
    if (buf.byteLength > 4096) {
      throw new RangeError("blockingWriteAndFlush accepts at most 4096 bytes");
    }
    if (this.handler.blockingWriteAndFlush) {
      return this.handler.blockingWriteAndFlush.call(this, buf);
    }
    this.handler.write.call(this, buf);
    if (this.handler.blockingFlush) {
      this.handler.blockingFlush.call(this);
    } else {
      this.handler.flush?.call(this);
    }
  }
  flush() {
    if (!this.open) {
      closed();
    }
    this.#permit = 0n;
    if (this.handler.flush) {
      this.handler.flush.call(this);
    }
  }
  blockingFlush() {
    if (!this.open) {
      closed();
    }
    if (this.handler.blockingFlush) {
      this.handler.blockingFlush.call(this);
    } else {
      this.handler.flush?.call(this);
    }
  }
  writeZeroes(len) {
    const length = checkedLength(len);
    if (len > this.#permit) {
      throw new Error("write exceeds the permit returned by checkWrite");
    }
    this.write.call(this, new Uint8Array(length));
  }
  blockingWriteZeroesAndFlush(len) {
    const length = checkedLength(len);
    if (length > 4096) {
      throw new RangeError("blockingWriteZeroesAndFlush accepts at most 4096 bytes");
    }
    this.blockingWriteAndFlush.call(this, new Uint8Array(length));
  }
  splice(src, len) {
    const spliceLen = Math.min(checkedLength(len), Number(this.checkWrite.call(this)));
    const bytes = src.read(BigInt(spliceLen));
    this.write.call(this, bytes);
    return BigInt(bytes.byteLength);
  }
  blockingSplice(src, len) {
    const spliceLen = Math.min(checkedLength(len), Number(this.checkWrite.call(this)));
    const bytes = src.blockingRead(BigInt(spliceLen));
    this.write.call(this, bytes);
    return BigInt(bytes.byteLength);
  }
  subscribe() {
    if (!this.open) {
      return pollableCreate();
    }
    const pollable = this.handler.subscribe ? this.handler.subscribe.call(this) : pollableCreate();
    if (pollable instanceof Pollable) {
      this.#children.add(pollable);
      pollable._onDispose(() => this.#children.delete(pollable));
    }
    return pollable;
  }
  [symbolDispose]() {
    if (!this.open) {
      return;
    }
    this.open = false;
    this.#permit = 0n;
    for (const child of this.#children) {
      child._invalidate();
    }
    this.#children.clear();
    this.handler.drop?.call(this);
  }
};
var outputStreamCreate = OutputStream._create;
delete OutputStream._create;
var error = {
  Error: IoError
};
var streams = { InputStream, OutputStream };
var Pollable = class _Pollable {
  #source = { ready: () => true, wait: () => Promise.resolve() };
  #invalid = false;
  #disposed = false;
  #wait = null;
  #disposeCallbacks = [];
  #wakeUnusable;
  #unusable = new Promise((resolve) => this.#wakeUnusable = resolve);
  static _create(source) {
    const pollable = new _Pollable();
    if (source instanceof Promise) {
      let ready = false;
      const wait = source.then(() => {
        ready = true;
      }, () => {
        ready = true;
      });
      pollable.#source = { ready: () => ready, wait: () => wait };
    } else if (source) {
      pollable.#source = source;
    }
    return pollable;
  }
  ready() {
    this.#assertUsable();
    return this.#source.ready();
  }
  block() {
    this.#assertUsable();
    if (this.#source.ready()) {
      return Promise.resolve();
    }
    if (!this.#wait) {
      this.#wait = Promise.race([
        Promise.resolve(this.#source.wait()),
        this.#unusable.then(() => this.#assertUsable())
      ]).finally(() => {
        this.#wait = null;
      });
    }
    return this.#wait;
  }
  _onDispose(callback) {
    if (this.#disposed) {
      callback();
    } else {
      this.#disposeCallbacks.push(callback);
    }
  }
  _invalidate() {
    if (this.#invalid || this.#disposed) {
      return;
    }
    this.#invalid = true;
    this.#wakeUnusable();
  }
  #assertUsable() {
    if (this.#disposed) {
      throw new Error("pollable has been disposed");
    }
    if (this.#invalid) {
      throw new Error("pollable's parent resource has been disposed");
    }
  }
  [symbolDispose]() {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#wakeUnusable();
    for (const callback of this.#disposeCallbacks.splice(0)) {
      callback();
    }
  }
};
var pollableCreate = Pollable._create;
delete Pollable._create;
function pollList(list) {
  if (list.length === 0) {
    throw new Error("poll list must not be empty");
  }
  if (list.length > 4294967295) {
    throw new Error("poll list length exceeds u32 index range");
  }
  const ready = [];
  for (let i = 0; i < list.length; i++) {
    if (list[i].ready()) {
      ready.push(i);
    }
  }
  if (ready.length > 0) {
    return new Promise((resolve) => setTimeout(() => {
      const result = [];
      for (let i = 0; i < list.length; i++) {
        if (list[i].ready()) {
          result.push(i);
        }
      }
      resolve(new Uint32Array(result));
    }, 0));
  }
  return Promise.race(list.map((pollable) => pollable.block())).then(() => {
    const result = [];
    for (let i = 0; i < list.length; i++) {
      if (list[i].ready()) {
        result.push(i);
      }
    }
    return new Uint32Array(result);
  });
}
function pollOne(poll2) {
  return poll2.block();
}
var poll = {
  Pollable,
  pollList,
  pollOne,
  // @ts-expect-error Not matching signature from WIT
  poll: pollList
};

// node_modules/@bytecodealliance/preview2-shim/dist/browser/config.js
var _cwd = "/";
function _setCwd(cwd) {
  _cwd = cwd;
}
function _getCwd() {
  return _cwd;
}

// node_modules/@bytecodealliance/preview2-shim/dist/browser/environment.js
var _env = [];
var _args = [];
var _cwd2 = "/";
function _setEnv(envObj) {
  _env = Object.entries(envObj);
}
function _setArgs(args) {
  _args = args;
}
var environment = {
  getEnvironment() {
    return _env;
  },
  getArguments() {
    return _args;
  },
  initialCwd() {
    return _cwd2;
  }
};

// node_modules/@bytecodealliance/preview2-shim/dist/browser/cli.js
var symbolDispose2 = Symbol.dispose ?? Symbol.for("dispose");
var ComponentExit = class extends Error {
  exitError = true;
  code;
  constructor(code) {
    super(`Component exited ${code === 0 ? "successfully" : "with error"}`);
    this.code = code;
  }
};
var exit = {
  exit(status) {
    throw new ComponentExit(status.tag === "err" ? 1 : 0);
  },
  // @ts-expect-error - Available only wasi-cli v0.2.12
  exitWithCode(code) {
    throw new ComponentExit(code);
  }
};
function _setStdin(handler) {
  stdinStream.handler = handler;
}
function _setStderr(handler) {
  stderrStream.handler = handler;
}
function _setStdout(handler) {
  stdoutStream.handler = handler;
}
var stdinStream = inputStreamCreate({
  blockingRead() {
    throw { tag: "closed" };
  },
  subscribe() {
    return pollableCreate();
  },
  [symbolDispose2]() {
  }
});
function consoleStream(writeLine) {
  const decoder = new TextDecoder();
  let pending = "";
  const emitCompleteLines = () => {
    const lines = pending.split("\n");
    pending = lines.pop();
    for (const line of lines) {
      writeLine(line.endsWith("\r") ? line.slice(0, -1) : line);
    }
  };
  return {
    write(contents) {
      pending += decoder.decode(contents, { stream: true });
      emitCompleteLines();
    },
    flush() {
      pending += decoder.decode();
      if (pending) {
        writeLine(pending);
      }
      pending = "";
    },
    blockingFlush() {
      this.flush?.();
    },
    drop() {
      this.flush?.();
    }
  };
}
var stdoutStream = outputStreamCreate(consoleStream((line) => console.log(line)));
var stderrStream = outputStreamCreate(consoleStream((line) => console.error(line)));
var stdin = {
  getStdin() {
    return stdinStream;
  }
};
var stdout = {
  getStdout() {
    return stdoutStream;
  }
};
var stderr = {
  getStderr() {
    return stderrStream;
  }
};
var TerminalInput = class {
};
var TerminalOutput = class {
};
var terminalInput = {
  TerminalInput
};
var terminalOutput = {
  TerminalOutput
};
var terminalStderr = {
  getTerminalStderr() {
    return void 0;
  }
};
var terminalStdin = {
  getTerminalStdin() {
    return void 0;
  }
};
var terminalStdout = {
  getTerminalStdout() {
    return void 0;
  }
};
function createCli(config = {}) {
  const stdinInstance = inputStreamCreate(config.stdin ?? {
    blockingRead() {
      throw { tag: "closed" };
    },
    subscribe: () => pollableCreate()
  });
  const stdoutInstance = outputStreamCreate(config.stdout ?? consoleStream((line) => console.log(line)));
  const stderrInstance = outputStreamCreate(config.stderr ?? consoleStream((line) => console.error(line)));
  const env = Object.entries(config.environment ?? {});
  const args = [...config.arguments ?? []];
  const cwd = config.initialCwd ?? "/";
  return {
    environment: {
      getEnvironment: () => env.map(([key, value]) => [key, value]),
      getArguments: () => [...args],
      initialCwd: () => cwd
    },
    exit,
    stdin: { getStdin: () => stdinInstance },
    stdout: { getStdout: () => stdoutInstance },
    stderr: { getStderr: () => stderrInstance },
    terminalInput,
    terminalOutput,
    terminalStdin,
    terminalStdout,
    terminalStderr
  };
}

// node_modules/@bytecodealliance/preview2-shim/dist/browser/clocks.js
var clocks_exports = {};
__export(clocks_exports, {
  monotonicClock: () => monotonicClock,
  wallClock: () => wallClock
});
var MAX_TIMEOUT_MS = 2147483647;
function timeout(durationNs) {
  let remainingMs = Number((durationNs + 999999n) / 1000000n);
  return new Promise((resolve) => {
    const next = () => {
      if (remainingMs <= 0) {
        resolve();
        return;
      }
      const delay = Math.min(remainingMs, MAX_TIMEOUT_MS);
      remainingMs -= delay;
      setTimeout(next, delay);
    };
    next();
  });
}
var monotonicClock = {
  resolution() {
    return BigInt(1e6);
  },
  now() {
    return BigInt(Math.floor(performance.now() * 1e6));
  },
  subscribeInstant(instant) {
    instant = checkedU64(instant, "instant");
    const now = monotonicClock.now();
    if (instant <= now) {
      return pollableCreate();
    }
    return monotonicClock.subscribeDuration(instant - now);
  },
  subscribeDuration(duration) {
    duration = checkedU64(duration, "duration");
    if (duration === 0n) {
      return pollableCreate();
    }
    return pollableCreate(timeout(duration));
  }
};
var wallClock = {
  now() {
    let now = Date.now();
    const seconds = BigInt(Math.floor(now / 1e3));
    const nanoseconds = now % 1e3 * 1e6;
    return { seconds, nanoseconds };
  },
  resolution() {
    return { seconds: 0n, nanoseconds: 1e6 };
  }
};

// node_modules/@bytecodealliance/preview2-shim/dist/browser/filesystem.js
var filesystem_exports = {};
__export(filesystem_exports, {
  InMemoryFilesystemAdapter: () => InMemoryFilesystemAdapter,
  OpfsFilesystemAdapter: () => OpfsFilesystemAdapter,
  _addPreopen: () => _addPreopen,
  _addPreopenWithAdapter: () => _addPreopenWithAdapter,
  _clearPreopens: () => _clearPreopens,
  _createPreopenDescriptor: () => _createPreopenDescriptor,
  _getFileData: () => _getFileData,
  _getPreopens: () => _getPreopens,
  _setCwd: () => _setCwd,
  _setFileData: () => _setFileData,
  _setPreopens: () => _setPreopens,
  createFilesystem: () => createFilesystem,
  loadOpfsCapability: () => loadOpfsCapability,
  preopens: () => preopens,
  types: () => types
});

// node_modules/@bytecodealliance/preview2-shim/dist/browser/in-memory-filesystem.js
var rootEntries = /* @__PURE__ */ new WeakSet();
var timeZero = {
  seconds: 0n,
  nanoseconds: 0
};
function coerceToSafeIntegerNumber(obj) {
  let n;
  if (typeof obj === "number") {
    n = obj;
  } else if (typeof obj == "bigint") {
    n = Number(obj);
  } else {
    throw new TypeError(`unexpected non-numeric type: ${obj}`);
  }
  if (n > Number.MAX_SAFE_INTEGER) {
    throw new TypeError(`excessively large number: ${n}`);
  }
  return n;
}
var MAX_SYMLINK_DEPTH = 40;
function resolveEntry(root, path, followFinal, allowMissingFinal = false) {
  const directories = [root];
  const pending = path.split("/").reverse();
  let followed = 0;
  while (pending.length) {
    const parent = directories[directories.length - 1];
    if (!parent.dir) {
      throw "not-directory";
    }
    const name = pending.pop();
    if (name === "" || name === ".") {
      continue;
    }
    if (name === "..") {
      if (directories.length === 1) {
        throw "not-permitted";
      }
      directories.pop();
      continue;
    }
    const entry2 = parent.dir[name];
    const isFinal = pending.length === 0;
    if (!entry2) {
      if (isFinal && allowMissingFinal) {
        return { entry: void 0, parent, name };
      }
      throw "no-entry";
    }
    if (entry2.symlink !== void 0 && (!isFinal || followFinal)) {
      if (++followed > MAX_SYMLINK_DEPTH) {
        throw "loop";
      }
      if (entry2.symlink.startsWith("/")) {
        throw "not-permitted";
      }
      for (const segment of entry2.symlink.split("/").reverse()) {
        pending.push(segment);
      }
      continue;
    }
    if (isFinal) {
      return { entry: entry2, parent, name };
    }
    directories.push(entry2);
  }
  const entry = directories[directories.length - 1];
  return { entry, parent: entry, name: "" };
}
function lookupPath(root, path) {
  if (path === "." && rootEntries.has(root)) {
    return _getCwd();
  }
  return path;
}
function getChildEntry(parentEntry, subpath, followFinal) {
  return resolveEntry(parentEntry, lookupPath(parentEntry, subpath), !!followFinal).entry;
}
function getParentEntry(root, path) {
  const segments = path.split("/").filter((segment) => segment !== "" && segment !== ".");
  const name = segments.pop();
  if (!name || name === "..") {
    throw "invalid";
  }
  const parent = resolveEntry(root, segments.join("/"), true).entry;
  if (!parent.dir) {
    throw "not-directory";
  }
  return [parent, name];
}
function getSource(fileEntry) {
  if (typeof fileEntry.source === "string") {
    fileEntry.source = new TextEncoder().encode(fileEntry.source);
  }
  return fileEntry.source;
}
function describeEntry(entry) {
  if (entry.symlink !== void 0) {
    return {
      type: "symbolic-link",
      size: BigInt(new TextEncoder().encode(entry.symlink).byteLength)
    };
  }
  if (entry.dir) {
    return { type: "directory", size: 0n };
  }
  return { type: "regular-file", size: BigInt(getSource(entry).byteLength) };
}
function containsEntry(root, target) {
  if (root === target) {
    return true;
  }
  return root.dir ? Object.values(root.dir).some((entry) => containsEntry(entry, target)) : false;
}
var fileWriteBuffers = /* @__PURE__ */ new WeakMap();
var nextEntryId = 0n;
var entryMetadata = /* @__PURE__ */ new WeakMap();
function metadata(entry) {
  let value = entryMetadata.get(entry);
  if (!value) {
    value = { id: ++nextEntryId, version: 0n, linkCount: 1n };
    entryMetadata.set(entry, value);
  }
  return value;
}
var fileLocks = /* @__PURE__ */ new WeakMap();
function lockState(entry) {
  let state = fileLocks.get(entry);
  if (!state) {
    state = { exclusiveHolder: null, sharedHolders: /* @__PURE__ */ new Set() };
    fileLocks.set(entry, state);
  }
  return state;
}
var touchListeners = /* @__PURE__ */ new Set();
function _onTouch(listener) {
  touchListeners.add(listener);
  return () => touchListeners.delete(listener);
}
function touch(entry) {
  metadata(entry).version++;
  for (const listener of touchListeners) {
    listener(entry);
  }
}
function getFileWriteBuffer(entry, source, requiredLength) {
  let buffer = fileWriteBuffers.get(entry);
  if (!buffer || buffer.buffer !== source.buffer || buffer.byteOffset !== source.byteOffset) {
    buffer = source;
  }
  if (requiredLength <= buffer.byteLength) {
    return buffer;
  }
  const newBuffer = new Uint8Array(Math.max(requiredLength, source.byteLength * 2));
  newBuffer.set(source);
  fileWriteBuffers.set(entry, newBuffer);
  return newBuffer;
}
var DirectoryEntryStream = class _DirectoryEntryStream {
  idx = 0;
  entries = [];
  static _create(entries) {
    const stream = new _DirectoryEntryStream();
    stream.entries = entries;
    return stream;
  }
  readDirectoryEntry() {
    if (this.idx === this.entries.length) {
      return void 0;
    }
    const [name, entry] = this.entries[this.idx];
    this.idx += 1;
    return {
      name,
      type: describeEntry(entry).type
    };
  }
};
var descriptorEntryStreamCreate = DirectoryEntryStream._create;
delete DirectoryEntryStream._create;
var Descriptor = class _Descriptor {
  #stream;
  #entry;
  #flags = {
    read: true,
    write: true,
    mutateDirectory: true
  };
  #advice = "normal";
  _getEntry(descriptor) {
    return descriptor.#entry;
  }
  static _create(entry, isStream) {
    const descriptor = new _Descriptor();
    if (isStream) {
      descriptor.#stream = entry;
    } else {
      descriptor.#entry = entry;
    }
    return descriptor;
  }
  readViaStream(_offset) {
    const source = getSource(this.#entry);
    let offset = Number(_offset);
    return inputStreamCreate({
      blockingRead(len) {
        if (offset === source.byteLength) {
          throw { tag: "closed" };
        }
        const bytes = source.slice(offset, offset + Number(len));
        offset += bytes.byteLength;
        return bytes;
      }
    });
  }
  writeViaStream(_offset) {
    const entry = this.#entry;
    let offset = coerceToSafeIntegerNumber(_offset);
    return outputStreamCreate({
      write(buf) {
        if (buf.byteLength === 0) {
          return;
        }
        const source = getSource(entry);
        const end = offset + buf.byteLength;
        if (!Number.isSafeInteger(end)) {
          throw new TypeError(`excessively large number: ${end}`);
        }
        const buffer = getFileWriteBuffer(entry, source, end);
        if (offset > source.byteLength) {
          buffer.fill(0, source.byteLength, offset);
        }
        buffer.set(buf, offset);
        entry.source = buffer.subarray(0, Math.max(source.byteLength, end));
        offset = end;
        touch(entry);
      }
    });
  }
  appendViaStream() {
    return this.writeViaStream(this.stat().size);
  }
  advise(_offset, _length, advice) {
    if (this.getType() === "directory") {
      throw "bad-descriptor";
    }
    this.#advice = advice;
  }
  syncData() {
  }
  getFlags() {
    return { ...this.#flags };
  }
  getType() {
    if (this.#stream) {
      return "fifo";
    }
    if (this.#entry.symlink !== void 0) {
      return "symbolic-link";
    }
    if (this.#entry.dir) {
      return "directory";
    }
    if (this.#entry.source) {
      return "regular-file";
    }
    return "unknown";
  }
  setSize(size) {
    if (this.getType() === "directory") {
      throw "is-directory";
    }
    const length = coerceToSafeIntegerNumber(size);
    const source = getSource(this.#entry);
    const resized = new Uint8Array(length);
    resized.set(source.subarray(0, length));
    this.#entry.source = resized;
    touch(this.#entry);
  }
  setTimes(dataAccessTimestamp, dataModificationTimestamp) {
    if (dataAccessTimestamp?.tag !== "no-change" || dataModificationTimestamp?.tag !== "no-change") {
      touch(this.#entry);
    }
  }
  read(length, offset) {
    const source = getSource(this.#entry);
    const off = coerceToSafeIntegerNumber(offset);
    const len = coerceToSafeIntegerNumber(length);
    const result = [
      source.slice(off, off + len),
      off + len >= source.byteLength
    ];
    return result;
  }
  write(buffer, offset) {
    if (this.getType() === "directory") {
      throw "is-directory";
    }
    const off = coerceToSafeIntegerNumber(offset);
    const source = getSource(this.#entry);
    const end = off + buffer.byteLength;
    if (!Number.isSafeInteger(end)) {
      throw "file-too-large";
    }
    const target = new Uint8Array(Math.max(source.byteLength, end));
    target.set(source);
    target.set(buffer, off);
    this.#entry.source = target;
    touch(this.#entry);
    return BigInt(buffer.byteLength);
  }
  readDirectory() {
    if (!this.#entry?.dir) {
      throw "bad-descriptor";
    }
    return descriptorEntryStreamCreate(Object.entries(this.#entry.dir).sort(([a], [b]) => a > b ? 1 : -1));
  }
  sync() {
  }
  createDirectoryAt(path) {
    try {
      getChildEntry(this.#entry, path, false);
      throw "exist";
    } catch (error2) {
      if (error2 !== "no-entry") {
        throw error2;
      }
    }
    const [parent, name] = getParentEntry(this.#entry, path);
    parent.dir[name] = { dir: {} };
    touch(parent);
  }
  stat() {
    const { type, size } = describeEntry(this.#entry);
    return {
      type,
      linkCount: metadata(this.#entry).linkCount,
      size,
      dataAccessTimestamp: timeZero,
      dataModificationTimestamp: timeZero,
      statusChangeTimestamp: timeZero
    };
  }
  statAt(pathFlags, path) {
    const entry = getChildEntry(this.#entry, path, pathFlags.symlinkFollow);
    const { type, size } = describeEntry(entry);
    return {
      type,
      linkCount: metadata(entry).linkCount,
      size,
      dataAccessTimestamp: timeZero,
      dataModificationTimestamp: timeZero,
      statusChangeTimestamp: timeZero
    };
  }
  setTimesAt(pathFlags, path, _atime, mtime) {
    const entry = getChildEntry(this.#entry, path, pathFlags.symlinkFollow);
    if (mtime?.tag !== "no-change") {
      fileWriteBuffers.delete(entry);
      touch(entry);
    }
  }
  linkAt(oldPathFlags, oldPath, newDescriptor, newPath) {
    const entry = getChildEntry(this.#entry, oldPath, oldPathFlags.symlinkFollow);
    if (entry.dir) {
      throw "not-permitted";
    }
    const [newParent, newName] = getParentEntry(descriptorGetEntry(unwrapDescriptor(newDescriptor)), newPath);
    if (newParent.dir[newName]) {
      throw "exist";
    }
    newParent.dir[newName] = entry;
    metadata(entry).linkCount++;
    touch(newParent);
  }
  openAt(pathFlags, path, openFlags, _flags) {
    const exclusiveCreate = !!(openFlags.create && openFlags.exclusive);
    const resolved = resolveEntry(this.#entry, lookupPath(this.#entry, path), !!pathFlags.symlinkFollow && !exclusiveCreate, !!openFlags.create);
    let childEntry = resolved.entry;
    if (childEntry && exclusiveCreate) {
      throw "exist";
    }
    if (!childEntry) {
      const { parent, name } = resolved;
      childEntry = parent.dir[name] = openFlags.directory ? { dir: {} } : { source: new Uint8Array() };
      touch(parent);
    }
    if (childEntry.symlink !== void 0) {
      throw "loop";
    }
    if (openFlags.directory && !childEntry.dir) {
      throw "not-directory";
    }
    if (openFlags.truncate) {
      if (childEntry.dir) {
        throw "is-directory";
      }
      childEntry.source = new Uint8Array();
      touch(childEntry);
    }
    return descriptorCreate(childEntry);
  }
  readlinkAt(path) {
    const entry = getChildEntry(this.#entry, path, false);
    if (entry.symlink === void 0) {
      throw "invalid";
    }
    if (entry.symlink.startsWith("/")) {
      throw "not-permitted";
    }
    return entry.symlink;
  }
  removeDirectoryAt(path) {
    const [parent, name] = getParentEntry(this.#entry, path);
    const entry = parent.dir?.[name];
    if (!entry) {
      throw "no-entry";
    }
    if (!entry.dir) {
      throw "not-directory";
    }
    if (Object.keys(entry.dir).length) {
      throw "not-empty";
    }
    delete parent.dir[name];
    metadata(entry).linkCount--;
    touch(parent);
  }
  renameAt(oldPath, newDescriptor, newPath) {
    const [oldParent, oldName] = getParentEntry(this.#entry, oldPath);
    const entry = oldParent.dir?.[oldName];
    if (!entry) {
      throw "no-entry";
    }
    const [newParent, newName] = getParentEntry(descriptorGetEntry(unwrapDescriptor(newDescriptor)), newPath);
    const replaced = newParent.dir[newName];
    if (oldParent === newParent && oldName === newName || replaced === entry) {
      return;
    }
    if (entry.dir && containsEntry(entry, newParent)) {
      throw "invalid";
    }
    if (replaced) {
      if (entry.dir && !replaced.dir) {
        throw "not-directory";
      }
      if (!entry.dir && replaced.dir) {
        throw "is-directory";
      }
      if (replaced.dir && Object.keys(replaced.dir).length > 0) {
        throw "not-empty";
      }
      metadata(replaced).linkCount--;
    }
    newParent.dir[newName] = entry;
    delete oldParent.dir[oldName];
    touch(oldParent);
    if (newParent !== oldParent) {
      touch(newParent);
    }
  }
  symlinkAt(oldPath, newPath) {
    if (oldPath.startsWith("/")) {
      throw "not-permitted";
    }
    const [parent, name] = getParentEntry(this.#entry, newPath);
    if (parent.dir[name]) {
      throw "exist";
    }
    parent.dir[name] = { symlink: oldPath };
    touch(parent);
  }
  unlinkFileAt(path) {
    const [parent, name] = getParentEntry(this.#entry, path);
    const entry = parent.dir?.[name];
    if (!entry) {
      throw "no-entry";
    }
    if (entry.dir) {
      throw "is-directory";
    }
    delete parent.dir[name];
    metadata(entry).linkCount--;
    touch(parent);
  }
  isSameObject(other) {
    return descriptorGetEntry(unwrapDescriptor(other)) === this.#entry;
  }
  metadataHash() {
    const value = metadata(this.#entry);
    return { upper: value.id, lower: value.version };
  }
  metadataHashAt(pathFlags, path) {
    const value = metadata(getChildEntry(this.#entry, path, pathFlags.symlinkFollow));
    return { upper: value.id, lower: value.version };
  }
  /**
   * Default advisory-locking implementation: a same-process reader/writer lock
   * keyed on the underlying entry. There's no real contention to wait out in a
   * single-threaded environment, so `lockShared`/`lockExclusive` don't block -
   * they throw `would-block` immediately when the lock isn't free, same as the
   * `tryLock*` variants report `false`.
   */
  tryLockShared() {
    const state = lockState(this.#entry);
    if (state.exclusiveHolder && state.exclusiveHolder !== this) {
      return false;
    }
    state.sharedHolders.add(this);
    return true;
  }
  tryLockExclusive() {
    const state = lockState(this.#entry);
    if (state.exclusiveHolder && state.exclusiveHolder !== this) {
      return false;
    }
    const otherReaders = state.sharedHolders.size - (state.sharedHolders.has(this) ? 1 : 0);
    if (otherReaders > 0) {
      return false;
    }
    state.sharedHolders.delete(this);
    state.exclusiveHolder = this;
    return true;
  }
  lockShared() {
    if (!this.tryLockShared()) {
      throw "would-block";
    }
  }
  lockExclusive() {
    if (!this.tryLockExclusive()) {
      throw "would-block";
    }
  }
  unlock() {
    const state = fileLocks.get(this.#entry);
    if (!state) {
      return;
    }
    state.sharedHolders.delete(this);
    if (state.exclusiveHolder === this) {
      state.exclusiveHolder = null;
    }
  }
};
var descriptorGetEntry = Descriptor.prototype._getEntry;
delete Descriptor.prototype._getEntry;
var descriptorCreate = Descriptor._create;
delete Descriptor._create;
var UNWRAP_DESCRIPTOR = Symbol("browserFilesystemDescriptor.unwrap");
function unwrapDescriptor(descriptor) {
  let current = descriptor;
  for (; ; ) {
    const inner = current[UNWRAP_DESCRIPTOR];
    if (!inner || inner === current) {
      return current;
    }
    current = inner;
  }
}
var InMemoryFilesystemAdapter = class {
  getRoot(capability) {
    if (!capability.dir) {
      throw new TypeError("an in-memory preopen root must be a directory");
    }
    rootEntries.add(capability);
    return descriptorCreate(capability);
  }
};

// node_modules/@bytecodealliance/preview2-shim/dist/browser/opfs-filesystem.js
var SYMLINKS_FILE = ".__wasi_symlinks__.json";
async function readSymlinksFile(handle) {
  try {
    const fileHandle = await handle.getFileHandle(SYMLINKS_FILE);
    const file = await fileHandle.getFile();
    return JSON.parse(await file.text());
  } catch {
    return {};
  }
}
async function writeSymlinksFile(handle, symlinks) {
  if (Object.keys(symlinks).length === 0) {
    try {
      await handle.removeEntry(SYMLINKS_FILE);
    } catch {
    }
    return;
  }
  const fileHandle = await handle.getFileHandle(SYMLINKS_FILE, { create: true });
  const writable = await fileHandle.createWritable();
  await writable.write(JSON.stringify(symlinks));
  await writable.close();
}
function collectSymlinks(dir, prefix, out) {
  for (const [name, entry] of Object.entries(dir)) {
    const path = prefix ? `${prefix}/${name}` : name;
    if (entry.symlink !== void 0) {
      out[path] = entry.symlink;
    } else if (entry.dir) {
      collectSymlinks(entry.dir, path, out);
    }
  }
}
function injectSymlinks(root, symlinks) {
  for (const [path, target] of Object.entries(symlinks)) {
    const parts = path.split("/").filter(Boolean);
    const name = parts.pop();
    if (!name) {
      continue;
    }
    let dir = root;
    for (const part of parts) {
      dir = dir[part]?.dir;
      if (!dir) {
        break;
      }
    }
    if (dir && !(name in dir)) {
      dir[name] = { symlink: target };
    }
  }
}
async function readEntries(handle) {
  const dir = {};
  for await (const [name, child] of handle.entries()) {
    if (name === SYMLINKS_FILE) {
      continue;
    }
    if (child.kind === "directory") {
      dir[name] = { dir: await readEntries(child) };
    } else {
      const file = await child.getFile();
      dir[name] = { source: new Uint8Array(await file.arrayBuffer()) };
    }
  }
  return dir;
}
async function writeEntries(handle, dir) {
  const seen = new Set(Object.keys(dir));
  seen.add(SYMLINKS_FILE);
  for await (const [name] of handle.entries()) {
    if (!seen.has(name)) {
      await handle.removeEntry(name, { recursive: true });
    }
  }
  for (const [name, entry] of Object.entries(dir)) {
    if (entry.symlink !== void 0) {
      continue;
    }
    if (entry.dir) {
      await writeEntries(await handle.getDirectoryHandle(name, { create: true }), entry.dir);
      continue;
    }
    const fileHandle = await handle.getFileHandle(name, { create: true });
    const writable = await fileHandle.createWritable();
    const source = entry.source ?? new Uint8Array();
    const bytes = typeof source === "string" ? new TextEncoder().encode(source) : source;
    await writable.write(bytes.slice());
    await writable.close();
  }
}
async function loadOpfsCapability(handle) {
  const dir = await readEntries(handle);
  injectSymlinks(dir, await readSymlinksFile(handle));
  return { handle, data: { dir } };
}
var LOCK_METHODS = /* @__PURE__ */ new Set([
  "lockShared",
  "lockExclusive",
  "tryLockShared",
  "tryLockExclusive",
  "unlock"
]);
function lockModeOf(prop) {
  return prop.endsWith("Exclusive") ? "exclusive" : "shared";
}
function joinLockPath(base, segment) {
  let path = base;
  for (const part of segment.split("/")) {
    if (part === "" || part === ".") {
      continue;
    }
    if (part === "..") {
      path = path.slice(0, path.lastIndexOf("/"));
      continue;
    }
    path = path ? `${path}/${part}` : part;
  }
  return path;
}
function withCrossTabLocking(descriptor, lockName, lockManager) {
  let release = null;
  function requestCrossTabLock(mode) {
    if (!lockManager) {
      return;
    }
    lockManager.request(lockName, { mode }, () => new Promise((resolve) => release = resolve)).catch(() => {
    });
  }
  function releaseCrossTabLock() {
    release?.();
    release = null;
  }
  return new Proxy(descriptor, {
    get(target, prop, receiver) {
      if (prop === UNWRAP_DESCRIPTOR) {
        return target;
      }
      const value = Reflect.get(target, prop, receiver);
      if (prop === "openAt") {
        return (...args) => {
          const child = Reflect.apply(value, target, args);
          return withCrossTabLocking(child, joinLockPath(lockName, args[1]), lockManager);
        };
      }
      if (typeof prop === "string" && LOCK_METHODS.has(prop) && typeof value === "function") {
        return (...args) => {
          const result = Reflect.apply(value, target, args);
          if (prop === "unlock") {
            releaseCrossTabLock();
          } else if (prop.startsWith("tryLock")) {
            if (result) {
              requestCrossTabLock(lockModeOf(prop));
            }
          } else {
            requestCrossTabLock(lockModeOf(prop));
          }
          return result;
        };
      }
      return typeof value === "function" ? value.bind(target) : value;
    }
  });
}
var OpfsFilesystemAdapter = class {
  #inMemory = new InMemoryFilesystemAdapter();
  #roots = [];
  #lockManager;
  #flushScheduled = false;
  #unsubscribeTouch;
  constructor(options = {}) {
    this.#lockManager = options.lockManager;
    this.#unsubscribeTouch = _onTouch(() => this.#scheduleFlush());
  }
  getRoot(capability) {
    this.#roots.push(capability);
    const descriptor = this.#inMemory.getRoot(capability.data);
    return this.#lockManager ? withCrossTabLocking(descriptor, capability.handle.name, this.#lockManager) : descriptor;
  }
  #scheduleFlush() {
    if (this.#flushScheduled) {
      return;
    }
    this.#flushScheduled = true;
    queueMicrotask(() => {
      this.#flushScheduled = false;
      void this.flush();
    });
  }
  /** Persist every loaded root's current in-memory state back to OPFS. */
  async flush() {
    await Promise.all(this.#roots.map(async (root) => {
      const dir = root.data.dir ?? {};
      await writeEntries(root.handle, dir);
      const symlinks = {};
      collectSymlinks(dir, "", symlinks);
      await writeSymlinksFile(root.handle, symlinks);
    }));
  }
  dispose() {
    this.#unsubscribeTouch();
    void this.flush();
  }
};

// node_modules/@bytecodealliance/preview2-shim/dist/browser/filesystem.js
var DirectoryEntryStream2 = class _DirectoryEntryStream {
  #implementation;
  static _create(implementation) {
    const stream = new _DirectoryEntryStream();
    stream.#implementation = implementation;
    return stream;
  }
  readDirectoryEntry() {
    return this.#implementation.readDirectoryEntry();
  }
};
var directoryEntryStreamCreate = DirectoryEntryStream2._create;
delete DirectoryEntryStream2._create;
var Descriptor2 = class _Descriptor {
  #implementation;
  _getImplementation(descriptor) {
    return descriptor.#implementation;
  }
  static _create(implementation) {
    const descriptor = new _Descriptor();
    descriptor.#implementation = implementation;
    return descriptor;
  }
  readViaStream(offset) {
    return this.#implementation.readViaStream(offset);
  }
  writeViaStream(offset) {
    return this.#implementation.writeViaStream(offset);
  }
  appendViaStream() {
    return this.#implementation.appendViaStream();
  }
  advise(offset, length, advice) {
    return this.#implementation.advise(offset, length, advice);
  }
  syncData() {
    return this.#implementation.syncData();
  }
  getFlags() {
    return this.#implementation.getFlags();
  }
  getType() {
    return this.#implementation.getType();
  }
  setSize(size) {
    return this.#implementation.setSize(size);
  }
  setTimes(dataAccessTimestamp, dataModificationTimestamp) {
    return this.#implementation.setTimes(dataAccessTimestamp, dataModificationTimestamp);
  }
  read(length, offset) {
    return this.#implementation.read(length, offset);
  }
  write(buffer, offset) {
    return this.#implementation.write(buffer, offset);
  }
  readDirectory() {
    return directoryEntryStreamCreate(this.#implementation.readDirectory());
  }
  sync() {
    return this.#implementation.sync();
  }
  createDirectoryAt(path) {
    return this.#implementation.createDirectoryAt(path);
  }
  stat() {
    return this.#implementation.stat();
  }
  statAt(pathFlags, path) {
    return this.#implementation.statAt(pathFlags, path);
  }
  setTimesAt(pathFlags, path, dataAccessTimestamp, dataModificationTimestamp) {
    return this.#implementation.setTimesAt(pathFlags, path, dataAccessTimestamp, dataModificationTimestamp);
  }
  linkAt(oldPathFlags, oldPath, newDescriptor, newPath) {
    return this.#implementation.linkAt(oldPathFlags, oldPath, descriptorGetImplementation(newDescriptor), newPath);
  }
  openAt(pathFlags, path, openFlags, flags) {
    return descriptorCreate2(this.#implementation.openAt(pathFlags, path, openFlags, flags));
  }
  readlinkAt(path) {
    return this.#implementation.readlinkAt(path);
  }
  removeDirectoryAt(path) {
    return this.#implementation.removeDirectoryAt(path);
  }
  renameAt(oldPath, newDescriptor, newPath) {
    return this.#implementation.renameAt(oldPath, descriptorGetImplementation(newDescriptor), newPath);
  }
  symlinkAt(oldPath, newPath) {
    return this.#implementation.symlinkAt(oldPath, newPath);
  }
  unlinkFileAt(path) {
    return this.#implementation.unlinkFileAt(path);
  }
  isSameObject(other) {
    return this.#implementation.isSameObject(descriptorGetImplementation(other));
  }
  metadataHash() {
    return this.#implementation.metadataHash();
  }
  metadataHashAt(pathFlags, path) {
    return this.#implementation.metadataHashAt(pathFlags, path);
  }
  lockShared() {
    return this.#implementation.lockShared?.();
  }
  lockExclusive() {
    return this.#implementation.lockExclusive?.();
  }
  tryLockShared() {
    return this.#implementation.tryLockShared?.() ?? false;
  }
  tryLockExclusive() {
    return this.#implementation.tryLockExclusive?.() ?? false;
  }
  unlock() {
    return this.#implementation.unlock?.();
  }
};
var descriptorGetImplementation = Descriptor2.prototype._getImplementation;
delete Descriptor2.prototype._getImplementation;
var descriptorCreate2 = Descriptor2._create;
delete Descriptor2._create;
var defaultAdapter = new InMemoryFilesystemAdapter();
var _fileData = { dir: {} };
var _preopens = [];
var _rootPreopen = null;
var preopens = {
  getDirectories() {
    return _preopens;
  }
};
function createFilesystem({ adapter, preopens: configuredPreopens }) {
  const entries = Object.entries(configuredPreopens).map(([guestPath, capability]) => [descriptorCreate2(adapter.getRoot(capability)), guestPath]);
  let disposed = false;
  return {
    types,
    preopens: {
      getDirectories() {
        if (disposed) {
          throw new Error("filesystem adapter has been disposed");
        }
        return [...entries];
      }
    },
    dispose() {
      if (disposed) {
        return;
      }
      disposed = true;
      adapter.dispose?.();
    }
  };
}
function _setFileData(fileData) {
  _fileData = fileData;
  if (_rootPreopen) {
    _rootPreopen[0] = descriptorCreate2(defaultAdapter.getRoot(fileData));
  } else {
    _setPreopens({ "/": fileData });
  }
  const cwd = environment.initialCwd();
  _setCwd(cwd || "/");
}
function _getFileData() {
  return JSON.stringify(_fileData);
}
function _setPreopens(preopensConfig) {
  _preopens = [];
  _rootPreopen = null;
  for (const [virtualPath, fileData] of Object.entries(preopensConfig)) {
    _addPreopen(virtualPath, fileData);
  }
}
function _addPreopen(virtualPath, fileData) {
  const descriptor = descriptorCreate2(defaultAdapter.getRoot(fileData));
  const entry = [descriptor, virtualPath];
  _preopens.push(entry);
  if (virtualPath === "/") {
    _rootPreopen = entry;
  }
}
function _addPreopenWithAdapter(virtualPath, adapter, capability) {
  const descriptor = descriptorCreate2(adapter.getRoot(capability));
  const entry = [descriptor, virtualPath];
  _preopens.push(entry);
  if (virtualPath === "/") {
    _rootPreopen = entry;
  }
}
function _clearPreopens() {
  _preopens = [];
  _rootPreopen = null;
}
function _getPreopens() {
  return [..._preopens];
}
function _createPreopenDescriptor(hostPreopen) {
  throw new TypeError(`browser preopen ${JSON.stringify(hostPreopen)} is a host path; configure browser file data or an adapter instead`);
}
var types = {
  Descriptor: Descriptor2,
  DirectoryEntryStream: DirectoryEntryStream2,
  filesystemErrorCode: (err) => {
    let message;
    if ("payload" in err) {
      message = err.payload;
    } else if ("message" in err) {
      message = err.message;
    }
    return convertFsError(message);
  }
};
function convertFsError(e) {
  switch (e.code) {
    case "EACCES":
      return "access";
    case "EAGAIN":
    case "EWOULDBLOCK":
      return "would-block";
    case "EALREADY":
      return "already";
    case "EBADF":
      return "bad-descriptor";
    case "EBUSY":
      return "busy";
    case "EDEADLK":
      return "deadlock";
    case "EDQUOT":
      return "quota";
    case "EEXIST":
      return "exist";
    case "EFBIG":
      return "file-too-large";
    case "EILSEQ":
      return "illegal-byte-sequence";
    case "EINPROGRESS":
      return "in-progress";
    case "EINTR":
      return "interrupted";
    case "EINVAL":
      return "invalid";
    case "EIO":
      return "io";
    case "EISDIR":
      return "is-directory";
    case "ELOOP":
      return "loop";
    case "EMLINK":
      return "too-many-links";
    case "EMSGSIZE":
      return "message-size";
    case "ENAMETOOLONG":
      return "name-too-long";
    case "ENODEV":
      return "no-device";
    case "ENOENT":
      return "no-entry";
    case "ENOLCK":
      return "no-lock";
    case "ENOMEM":
      return "insufficient-memory";
    case "ENOSPC":
      return "insufficient-space";
    case "ENOTDIR":
    case "ERR_FS_EISDIR":
      return "not-directory";
    case "ENOTEMPTY":
      return "not-empty";
    case "ENOTRECOVERABLE":
      return "not-recoverable";
    case "ENOTSUP":
      return "unsupported";
    case "ENOTTY":
      return "no-tty";
    // windows gives this error for badly structured `//` reads
    // this seems like a slightly better error than unknown given
    // that it's a common footgun
    case -4094:
    case "ENXIO":
      return "no-such-device";
    case "EOVERFLOW":
      return "overflow";
    case "EPERM":
      return "not-permitted";
    case "EPIPE":
      return "pipe";
    case "EROFS":
      return "read-only";
    case "ESPIPE":
      return "invalid-seek";
    case "ETXTBSY":
      return "text-file-busy";
    case "EXDEV":
      return "cross-device";
    case "UNKNOWN":
      switch (e.errno) {
        case -4094:
          return "no-such-device";
        default:
          throw e;
      }
    default:
      throw e;
  }
}

// node_modules/@bytecodealliance/preview2-shim/dist/browser/http.js
var http_exports = {};
__export(http_exports, {
  InMemoryHttpClient: () => InMemoryHttpClient,
  _setRequestStreaming: () => _setRequestStreaming,
  createIncomingHandler: () => createIncomingHandler,
  handleIncomingRequest: () => handleIncomingRequest,
  incomingHandler: () => incomingHandler,
  outgoingHandler: () => outgoingHandler,
  types: () => types2
});

// node_modules/@bytecodealliance/preview2-shim/dist/browser/in-memory-http.js
var InMemoryHttpClient = class {
  #handler;
  constructor(handler) {
    this.#handler = typeof handler === "function" ? handler : handler.handle.bind(handler);
  }
  fetch(request) {
    return handleIncomingRequest(request, this.#handler);
  }
};

// node_modules/@bytecodealliance/preview2-shim/dist/browser/http.js
var symbolDispose3 = Symbol.dispose || Symbol.for("dispose");
var utf8Encoder = new TextEncoder();
var utf8Decoder = new TextDecoder();
var forbiddenHeaders = /* @__PURE__ */ new Set(["connection", "keep-alive", "host"]);
var DEFAULT_HTTP_TIMEOUT_NS = 600000000000n;
var TOKEN_RE = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/;
var FIELD_VALUE_RE = /^[\t\x20-\x7E\x80-\xFF]*$/;
var BRACKETED_IPV6_AUTHORITY_RE = /^\[([0-9A-Fa-f:.]+)\](?::([0-9]+))?$/;
var DNS_OR_IPV4_AUTHORITY_RE = /^([a-zA-Z0-9.-]+)(?::([0-9]+))?$/;
function validateHeaderName(name) {
  if (!TOKEN_RE.test(name)) {
    throw { tag: "invalid-syntax" };
  }
}
function validateHeaderValue(value) {
  const str = typeof value === "string" ? value : utf8Decoder.decode(value);
  if (!FIELD_VALUE_RE.test(str)) {
    throw { tag: "invalid-syntax" };
  }
}
var Fields = class _Fields {
  #immutable = false;
  #entries = [];
  #table = /* @__PURE__ */ new Map();
  static fromList(entries) {
    const fields = new _Fields();
    for (const [key, value] of entries) {
      fields.append(key, value);
    }
    return fields;
  }
  get(name) {
    const tableEntries = this.#table.get(name.toLowerCase());
    if (!tableEntries) {
      return [];
    }
    return tableEntries.map(([, v]) => v);
  }
  /**
   * WIT spec (https://github.com/WebAssembly/WASI/blob/91bb44c3c3a9b1e09187db23c85fa844d1fd6b15/proposals/http/wit/types.wit#L215-L223):
   *
   * > Set all of the values for a name. Clears any existing values for that
   * > name, if they have been set.
   * >
   * > Fails with `header-error.immutable` if the `fields` are immutable.
   * >
   * > Fails with `header-error.invalid-syntax` if the `field-name` or any of
   * > the `field-value`s are syntactically invalid.
   *
   * The existing-branch splice/reuse is an allocation optimization — values are cleared, not retained.
   */
  set(name, values) {
    if (this.#immutable) {
      throw { tag: "immutable" };
    }
    validateHeaderName(name);
    for (const value of values) {
      validateHeaderValue(value);
    }
    const lowercased = name.toLowerCase();
    if (forbiddenHeaders.has(lowercased)) {
      throw { tag: "forbidden" };
    }
    const tableEntries = this.#table.get(lowercased);
    if (tableEntries) {
      this.#entries = this.#entries.filter((entry) => !tableEntries.includes(entry));
      tableEntries.splice(0, tableEntries.length);
    } else {
      this.#table.set(lowercased, []);
    }
    const newTableEntries = this.#table.get(lowercased);
    for (const value of values) {
      const entry = [name, value];
      this.#entries.push(entry);
      newTableEntries.push(entry);
    }
  }
  has(name) {
    return this.#table.has(name.toLowerCase());
  }
  delete(name) {
    if (this.#immutable) {
      throw { tag: "immutable" };
    }
    const lowercased = name.toLowerCase();
    const tableEntries = this.#table.get(lowercased);
    if (tableEntries) {
      this.#entries = this.#entries.filter((entry) => !tableEntries.includes(entry));
      this.#table.delete(lowercased);
    }
  }
  append(name, value) {
    if (this.#immutable) {
      throw { tag: "immutable" };
    }
    validateHeaderName(name);
    validateHeaderValue(value);
    const lowercased = name.toLowerCase();
    if (forbiddenHeaders.has(lowercased)) {
      throw { tag: "forbidden" };
    }
    const entry = [name, value];
    this.#entries.push(entry);
    const tableEntries = this.#table.get(lowercased);
    if (tableEntries) {
      tableEntries.push(entry);
    } else {
      this.#table.set(lowercased, [entry]);
    }
  }
  entries() {
    return this.#entries;
  }
  clone() {
    return fieldsFromEntriesChecked(this.#entries);
  }
  static _lock(fields) {
    fields.#immutable = true;
    return fields;
  }
  static _fromEntriesChecked(entries) {
    const fields = new _Fields();
    fields.#entries = entries;
    for (const entry of entries) {
      const lowercase = entry[0].toLowerCase();
      const existing = fields.#table.get(lowercase);
      if (existing) {
        existing.push(entry);
      } else {
        fields.#table.set(lowercase, [entry]);
      }
    }
    return fields;
  }
};
var fieldsLock = Fields._lock;
delete Fields._lock;
var fieldsFromEntriesChecked = Fields._fromEntriesChecked;
delete Fields._fromEntriesChecked;
var RequestOptions = class {
  #connectTimeout;
  #firstByteTimeout;
  #betweenBytesTimeout;
  connectTimeout() {
    return this.#connectTimeout;
  }
  setConnectTimeout(duration) {
    if (duration !== void 0 && duration < 0n) {
      throw new Error("duration must not be negative");
    }
    this.#connectTimeout = duration;
  }
  firstByteTimeout() {
    return this.#firstByteTimeout;
  }
  setFirstByteTimeout(duration) {
    if (duration !== void 0 && duration < 0n) {
      throw new Error("duration must not be negative");
    }
    this.#firstByteTimeout = duration;
  }
  betweenBytesTimeout() {
    return this.#betweenBytesTimeout;
  }
  setBetweenBytesTimeout(duration) {
    if (duration !== void 0 && duration < 0n) {
      throw new Error("duration must not be negative");
    }
    this.#betweenBytesTimeout = duration;
  }
};
var OutgoingBody = class _OutgoingBody {
  #outputStream = null;
  #chunks = [];
  #finished = false;
  #resolveFinished;
  #finishedPromise = new Promise((resolve) => this.#resolveFinished = resolve);
  #requestStream = null;
  #requestStreamController = null;
  #requestStreamCancelled = false;
  write() {
    const outputStream = this.#outputStream;
    if (outputStream === null) {
      throw void 0;
    }
    this.#outputStream = null;
    return outputStream;
  }
  static finish(body, trailers) {
    if (trailers) {
      throw { tag: "internal-error", val: "trailers unsupported" };
    }
    if (body.#finished) {
      throw { tag: "internal-error", val: "body already finished" };
    }
    body.#finished = true;
    if (!body.#requestStreamCancelled) {
      body.#requestStreamController?.close();
    }
    body.#resolveFinished();
  }
  #bodyData() {
    if (this.#chunks.length === 0) {
      return null;
    }
    let totalLen = 0;
    for (const chunk of this.#chunks) {
      totalLen += chunk.byteLength;
    }
    const result = new Uint8Array(totalLen);
    let offset = 0;
    for (const chunk of this.#chunks) {
      result.set(chunk, offset);
      offset += chunk.byteLength;
    }
    return result;
  }
  static async _finishedBodyData(outgoingBody) {
    await outgoingBody.#finishedPromise;
    return outgoingBody.#bodyData();
  }
  static _requestBodyStream(outgoingBody) {
    if (outgoingBody.#requestStream === null) {
      outgoingBody.#requestStream = new ReadableStream({
        start(controller) {
          outgoingBody.#requestStreamController = controller;
          for (const chunk of outgoingBody.#chunks) {
            controller.enqueue(chunk);
          }
          outgoingBody.#chunks.length = 0;
          if (outgoingBody.#finished) {
            controller.close();
          }
        },
        cancel() {
          outgoingBody.#requestStreamCancelled = true;
        }
      });
    }
    return outgoingBody.#requestStream;
  }
  static _create() {
    const outgoingBody = new _OutgoingBody();
    const chunks = outgoingBody.#chunks;
    outgoingBody.#outputStream = outputStreamCreate({
      write(buf) {
        if (outgoingBody.#finished || outgoingBody.#requestStreamCancelled) {
          throw { tag: "closed" };
        }
        const chunk = new Uint8Array(buf);
        if (outgoingBody.#requestStreamController) {
          outgoingBody.#requestStreamController.enqueue(chunk);
        } else {
          chunks.push(chunk);
        }
      },
      blockingFlush() {
      },
      subscribe() {
        return pollableCreate();
      }
    });
    return outgoingBody;
  }
  [symbolDispose3]() {
  }
};
var outgoingBodyCreate = OutgoingBody._create;
delete OutgoingBody._create;
var outgoingBodyFinishedData = OutgoingBody._finishedBodyData;
delete OutgoingBody._finishedBodyData;
var outgoingBodyRequestStream = OutgoingBody._requestBodyStream;
delete OutgoingBody._requestBodyStream;
var OutgoingRequest = class {
  #method = { tag: "get" };
  #scheme = void 0;
  #pathWithQuery = void 0;
  #authority = void 0;
  #headers;
  #body;
  #bodyRequested = false;
  constructor(headers) {
    fieldsLock(headers);
    this.#headers = headers;
    this.#body = outgoingBodyCreate();
  }
  body() {
    if (this.#bodyRequested) {
      throw new Error("Body already requested");
    }
    this.#bodyRequested = true;
    return this.#body;
  }
  method() {
    return this.#method;
  }
  setMethod(method) {
    if (method.tag === "other" && method.val && !method.val.match(/^[a-zA-Z-]+$/)) {
      throw void 0;
    }
    this.#method = method;
  }
  pathWithQuery() {
    return this.#pathWithQuery;
  }
  setPathWithQuery(pathWithQuery) {
    if (pathWithQuery && !pathWithQuery.match(/^[a-zA-Z0-9.\-_~!$&'()*+,;=:@%?/]+$/)) {
      throw void 0;
    }
    this.#pathWithQuery = pathWithQuery;
  }
  scheme() {
    return this.#scheme;
  }
  setScheme(scheme) {
    if (scheme?.tag === "other" && scheme.val && !scheme.val.match(/^[a-zA-Z]+$/)) {
      throw void 0;
    }
    this.#scheme = scheme;
  }
  authority() {
    return this.#authority;
  }
  setAuthority(authority) {
    if (authority !== void 0) {
      const match = authority.startsWith("[") ? authority.match(BRACKETED_IPV6_AUTHORITY_RE) : authority.match(DNS_OR_IPV4_AUTHORITY_RE);
      if (!match || match[2] !== void 0 && Number(match[2]) > 65535) {
        throw void 0;
      }
      try {
        const parsed = new URL(`http://${authority}/`);
        if (parsed.username || parsed.password || !parsed.hostname) {
          throw void 0;
        }
      } catch {
        throw void 0;
      }
    }
    this.#authority = authority;
  }
  headers() {
    return this.#headers;
  }
  [symbolDispose3]() {
  }
  static _handle(request, options, config = {}) {
    const scheme = schemeString(request.#scheme);
    const method = "val" in request.#method ? request.#method.val : request.#method.tag;
    if (!request.#pathWithQuery) {
      throw { tag: "HTTP-request-URI-invalid" };
    }
    const url = `${scheme}//${request.#authority || ""}${request.#pathWithQuery}`;
    const headers = new Headers();
    for (const [key, value] of request.#headers.entries()) {
      const lowerKey = key.toLowerCase();
      if (!forbiddenHeaders.has(lowerKey)) {
        headers.append(key, utf8Decoder.decode(value));
      }
    }
    const bodyData = request.#bodyRequested ? config.streamingRequestBodies ? outgoingBodyRequestStream(request.#body) : outgoingBodyFinishedData(request.#body) : null;
    let timeoutMs = Number(DEFAULT_HTTP_TIMEOUT_NS / 1000000n);
    if (options) {
      const ct = options.connectTimeout?.() ?? DEFAULT_HTTP_TIMEOUT_NS;
      const fbt = options.firstByteTimeout?.() ?? DEFAULT_HTTP_TIMEOUT_NS;
      const minTimeout = ct < fbt ? ct : fbt;
      timeoutMs = Number(minTimeout / 1000000n);
    }
    return futureIncomingResponseCreate(url, method.toUpperCase(), headers, bodyData, timeoutMs);
  }
};
var outgoingRequestHandle = OutgoingRequest._handle;
delete OutgoingRequest._handle;
var IncomingBody = class _IncomingBody {
  #finished = false;
  #stream = null;
  stream() {
    if (!this.#stream) {
      throw void 0;
    }
    const stream = this.#stream;
    this.#stream = null;
    return stream;
  }
  static finish(incomingBody) {
    if (incomingBody.#finished) {
      throw new Error("incoming body already finished");
    }
    incomingBody.#finished = true;
    return futureTrailersCreate();
  }
  [symbolDispose3]() {
  }
  static _create(fetchResponse, bufferedBody) {
    const incomingBody = new _IncomingBody();
    let buffer = bufferedBody ?? null;
    let bufferOffset = 0;
    let done = bufferedBody !== void 0;
    let reader = null;
    let readPromise = null;
    let readError = null;
    let disposed = false;
    function ready() {
      return done || buffer !== null && bufferOffset < buffer.byteLength;
    }
    function startRead() {
      if (readPromise || ready()) {
        return;
      }
      if (!fetchResponse.body) {
        done = true;
        return;
      }
      reader ??= fetchResponse.body.getReader();
      const activeReader = reader;
      readPromise = (async () => {
        try {
          while (!done) {
            const result = await activeReader.read();
            if (disposed) {
              return;
            }
            if (result.done) {
              done = true;
            } else if (result.value.byteLength > 0) {
              buffer = result.value;
              bufferOffset = 0;
              return;
            }
          }
        } catch (cause) {
          done = true;
          if (!disposed) {
            readError = ioErrorCreate(cause instanceof Error ? cause.message : String(cause));
          }
        } finally {
          readPromise = null;
          if (done) {
            activeReader.releaseLock();
          }
        }
      })();
    }
    async function waitForReadable() {
      while (!ready()) {
        startRead();
        await readPromise;
      }
    }
    function read(len) {
      if (readError) {
        const error2 = readError;
        readError = null;
        throw { tag: "last-operation-failed", val: error2 };
      }
      if (done && (buffer === null || bufferOffset >= buffer.byteLength)) {
        throw { tag: "closed" };
      }
      if (buffer === null || bufferOffset >= buffer.byteLength) {
        startRead();
        return new Uint8Array(0);
      }
      const toRead = Math.min(Number(len), buffer.byteLength - bufferOffset);
      const slice = buffer.slice(bufferOffset, bufferOffset + toRead);
      bufferOffset += toRead;
      if (bufferOffset >= buffer.byteLength) {
        buffer = null;
        bufferOffset = 0;
        startRead();
      }
      return slice;
    }
    function blockingRead(len) {
      if (len === 0n || ready()) {
        return read(len);
      }
      return waitForReadable().then(() => read(len));
    }
    function blockingSkip(len) {
      const result = blockingRead(len);
      return result instanceof Promise ? result.then((bytes) => BigInt(bytes.byteLength)) : BigInt(result.byteLength);
    }
    incomingBody.#stream = inputStreamCreate({
      read,
      // WIT declares synchronous signatures; JSPI awaits these host results
      // for blocking imports. Preserve synchronous results for buffered bodies.
      blockingRead,
      blockingSkip,
      subscribe() {
        return pollableCreate({ ready, wait: waitForReadable });
      },
      drop() {
        disposed = true;
        done = true;
        buffer = null;
        readError = null;
        if (reader) {
          const activeReader = reader;
          void activeReader.cancel().catch(() => {
          }).finally(() => {
            activeReader.releaseLock();
          });
        }
      }
    });
    if (bufferedBody === void 0) {
      startRead();
    }
    return incomingBody;
  }
};
var incomingBodyCreate = IncomingBody._create;
delete IncomingBody._create;
var IncomingResponse = class _IncomingResponse {
  #headers;
  #status = 0;
  #body;
  status() {
    return this.#status;
  }
  headers() {
    return this.#headers;
  }
  consume() {
    if (this.#body === void 0) {
      throw void 0;
    }
    const body = this.#body;
    this.#body = void 0;
    return body;
  }
  [symbolDispose3]() {
  }
  static _create(fetchResponse) {
    const res = new _IncomingResponse();
    res.#status = fetchResponse.status;
    const headerEntries = [];
    const encoder = new TextEncoder();
    fetchResponse.headers.forEach((value, key) => {
      headerEntries.push([key, encoder.encode(value)]);
    });
    res.#headers = fieldsLock(fieldsFromEntriesChecked(headerEntries));
    res.#body = incomingBodyCreate(fetchResponse);
    return res;
  }
};
var incomingResponseCreate = IncomingResponse._create;
delete IncomingResponse._create;
var IncomingRequest = class _IncomingRequest {
  #request;
  #headers;
  #body;
  method() {
    const method = this.#request.method.toLowerCase();
    return { tag: method };
  }
  pathWithQuery() {
    const url = new URL(this.#request.url);
    return `${url.pathname}${url.search}`;
  }
  scheme() {
    const protocol = new URL(this.#request.url).protocol;
    if (protocol === "http:") {
      return { tag: "HTTP" };
    }
    if (protocol === "https:") {
      return { tag: "HTTPS" };
    }
    return { tag: "other", val: protocol.slice(0, -1) };
  }
  authority() {
    return new URL(this.#request.url).host;
  }
  headers() {
    return this.#headers;
  }
  consume() {
    if (!this.#body) {
      throw new Error("incoming request body already consumed");
    }
    const body = this.#body;
    this.#body = void 0;
    return body;
  }
  static _create(request, bufferedBody) {
    const incoming = new _IncomingRequest();
    incoming.#request = request.clone();
    const encoder = new TextEncoder();
    incoming.#headers = fieldsLock(fieldsFromEntriesChecked([...request.headers.entries()].map(([name, value]) => [
      name,
      encoder.encode(value)
    ])));
    incoming.#body = incomingBodyCreate(new Response(request.body), bufferedBody);
    return incoming;
  }
  static _toRequest(request) {
    return request.#request;
  }
};
var incomingRequestCreate = IncomingRequest._create;
delete IncomingRequest._create;
var incomingRequestToRequest = IncomingRequest._toRequest;
delete IncomingRequest._toRequest;
var OutgoingResponse = class {
  #headers;
  #status = 200;
  #body = outgoingBodyCreate();
  #bodyRequested = false;
  constructor(headers) {
    fieldsLock(headers);
    this.#headers = headers;
  }
  statusCode() {
    return this.#status;
  }
  setStatusCode(statusCode) {
    if (!Number.isInteger(statusCode) || statusCode < 100 || statusCode > 999) {
      throw new TypeError("invalid HTTP status code");
    }
    this.#status = statusCode;
  }
  headers() {
    return this.#headers;
  }
  body() {
    if (this.#bodyRequested) {
      throw new Error("outgoing response body already requested");
    }
    this.#bodyRequested = true;
    return this.#body;
  }
  static async _toResponse(response) {
    const headers = new Headers();
    for (const [name, value] of response.#headers.entries()) {
      headers.append(name, utf8Decoder.decode(value));
    }
    const body = response.#bodyRequested ? await outgoingBodyFinishedData(response.#body) : null;
    return new Response(body, {
      status: response.#status,
      headers
    });
  }
};
var outgoingResponseToResponse = OutgoingResponse._toResponse;
delete OutgoingResponse._toResponse;
var ResponseOutparam = class _ResponseOutparam {
  #used = false;
  #resolve;
  #reject;
  static set(param, response) {
    if (param.#used) {
      throw new Error("response outparam already set");
    }
    param.#used = true;
    if (response.tag === "ok") {
      void outgoingResponseToResponse(response.val).then(param.#resolve, param.#reject);
    } else {
      param.#resolve(new Response(`WASI HTTP handler error: ${JSON.stringify(response.val)}`, {
        status: 500
      }));
    }
  }
  static _isUsed(param) {
    return param.#used;
  }
  static _create() {
    const param = new _ResponseOutparam();
    const response = new Promise((resolve, reject) => {
      param.#resolve = resolve;
      param.#reject = reject;
    });
    return [param, response];
  }
};
var responseOutparamCreate = ResponseOutparam._create;
delete ResponseOutparam._create;
var responseOutparamIsUsed = ResponseOutparam._isUsed;
delete ResponseOutparam._isUsed;
var FutureTrailers = class _FutureTrailers {
  #requested = false;
  subscribe() {
    return pollableCreate();
  }
  get() {
    if (this.#requested) {
      return { tag: "err", val: void 0 };
    }
    this.#requested = true;
    return {
      tag: "ok",
      val: {
        tag: "ok",
        val: fieldsLock(fieldsFromEntriesChecked([]))
      }
    };
  }
  static _create() {
    return new _FutureTrailers();
  }
};
var futureTrailersCreate = FutureTrailers._create;
delete FutureTrailers._create;
function mapFetchError(err) {
  if (err.name === "AbortError") {
    return { tag: "connection-timeout" };
  }
  return { tag: "internal-error", val: err.message };
}
var FutureIncomingResponse = class _FutureIncomingResponse {
  #result = void 0;
  #promise = null;
  #controller = null;
  #settled = false;
  subscribe() {
    return pollableCreate(this.#promise);
  }
  get() {
    if (this.#result === void 0) {
      return void 0;
    }
    const result = this.#result;
    this.#result = { tag: "err" };
    if (result.tag === "ok" && result.val.tag === "ok") {
      this.#controller = null;
    }
    return result;
  }
  [symbolDispose3]() {
    if (!this.#settled) {
      this.#controller?.abort();
    }
    this.#controller = null;
    this.#promise = null;
  }
  static _create(url, method, headers, bodyData, timeoutMs) {
    const future = new _FutureIncomingResponse();
    const controller = new AbortController();
    future.#controller = controller;
    let timer;
    if (timeoutMs < Infinity) {
      timer = setTimeout(() => controller.abort(), timeoutMs);
    }
    future.#promise = Promise.resolve(bodyData).then((bodyData2) => {
      const init = {
        method,
        headers,
        signal: controller.signal
      };
      if (bodyData2 && method !== "GET" && method !== "HEAD") {
        init.body = bodyData2;
        if (bodyData2 instanceof ReadableStream) {
          init.duplex = "half";
        }
      }
      return globalThis.fetch(url, init);
    }).then((response) => {
      if (timer) {
        clearTimeout(timer);
      }
      future.#settled = true;
      future.#result = {
        tag: "ok",
        val: {
          tag: "ok",
          val: incomingResponseCreate(response)
        }
      };
    }, (err) => {
      if (timer) {
        clearTimeout(timer);
      }
      future.#settled = true;
      future.#result = {
        tag: "ok",
        val: {
          tag: "err",
          val: mapFetchError(err)
        }
      };
    });
    return future;
  }
};
var futureIncomingResponseCreate = FutureIncomingResponse._create;
delete FutureIncomingResponse._create;
function schemeString(scheme) {
  if (!scheme) {
    return "https:";
  }
  switch (scheme.tag) {
    case "HTTP":
      return "http:";
    case "HTTPS":
      return "https:";
    case "other":
      return scheme.val.toLowerCase() + ":";
  }
  return "https:";
}
function httpErrorCode(err) {
  if ("payload" in err) {
    return err.payload;
  }
  return {
    tag: "internal-error",
    val: "message" in err ? err.message : err.toDebugString()
  };
}
var requestStreamingEnabled = false;
function _setRequestStreaming(enabled) {
  if (typeof enabled !== "boolean") {
    throw new TypeError("request streaming setting must be a boolean");
  }
  requestStreamingEnabled = enabled;
}
var outgoingHandler = {
  handle(request, options) {
    return outgoingRequestHandle(request, options, {
      streamingRequestBodies: requestStreamingEnabled
    });
  }
};
var incomingHandler = {
  handle() {
    throw "not-supported";
  }
};
function createIncomingHandler(handler) {
  return {
    async handle(request, responseOut) {
      try {
        const response = await handler(incomingRequestToRequest(request));
        if (!(response instanceof Response)) {
          throw new TypeError("incoming HTTP handler must return a Response");
        }
        ResponseOutparam.set(responseOut, {
          tag: "ok",
          val: await responseToOutgoingResponse(response)
        });
      } catch (error2) {
        ResponseOutparam.set(responseOut, {
          tag: "err",
          val: {
            tag: "internal-error",
            val: error2 instanceof Error ? error2.message : String(error2)
          }
        });
      }
    }
  };
}
async function responseToOutgoingResponse(response) {
  const headers = [];
  response.headers.forEach((value, name) => headers.push([name, utf8Encoder.encode(value)]));
  const outgoing = new OutgoingResponse(fieldsFromEntriesChecked(headers));
  outgoing.setStatusCode(response.status);
  if (response.body) {
    const body = outgoing.body();
    const stream = body.write();
    const reader = response.body.getReader();
    for (let result = await reader.read(); !result.done; result = await reader.read()) {
      let offset = 0;
      while (offset < result.value.byteLength) {
        const permit = stream.checkWrite();
        if (permit === 0n) {
          await stream.subscribe().block();
          continue;
        }
        const length = Math.min(Number(permit), result.value.byteLength - offset);
        stream.write(result.value.subarray(offset, offset + length));
        offset += length;
      }
    }
    OutgoingBody.finish(body, void 0);
  }
  return outgoing;
}
async function handleIncomingRequest(request, handler) {
  const [responseOut, response] = responseOutparamCreate();
  const body = request.body ? new Uint8Array(await request.clone().arrayBuffer()) : new Uint8Array();
  await handler(incomingRequestCreate(request, body), responseOut);
  if (!responseOutparamIsUsed(responseOut)) {
    throw new Error("WASI HTTP handler returned without setting its response outparam");
  }
  return response;
}
var types2 = {
  Fields,
  FutureIncomingResponse,
  FutureTrailers,
  IncomingBody,
  IncomingRequest,
  IncomingResponse,
  OutgoingBody,
  OutgoingRequest,
  OutgoingResponse,
  ResponseOutparam,
  RequestOptions,
  httpErrorCode
};

// node_modules/@bytecodealliance/preview2-shim/dist/browser/random.js
var random_exports = {};
__export(random_exports, {
  insecure: () => insecure,
  insecureSeed: () => insecureSeed,
  random: () => random
});
var MAX_BYTES = 65536;
var insecureRandomValue1;
var insecureRandomValue2;
var insecure = {
  getInsecureRandomBytes(len) {
    return random.getRandomBytes(len);
  },
  getInsecureRandomU64() {
    return random.getRandomU64();
  }
};
var insecureSeedValue1;
var insecureSeedValue2;
var insecureSeed = {
  insecureSeed() {
    if (insecureSeedValue1 === void 0 || insecureSeedValue2 === void 0) {
      insecureSeedValue1 = random.getRandomU64();
      insecureSeedValue2 = random.getRandomU64();
    }
    return [insecureSeedValue1, insecureSeedValue2];
  }
};
var random = {
  getRandomBytes(len) {
    const byteLength = checkedU64AsNumber(len, "random byte length");
    const bytes = new Uint8Array(byteLength);
    if (byteLength > MAX_BYTES) {
      for (let generated = 0; generated < byteLength; generated += MAX_BYTES) {
        crypto.getRandomValues(bytes.subarray(generated, generated + MAX_BYTES));
      }
    } else {
      crypto.getRandomValues(bytes);
    }
    return bytes;
  },
  getRandomU64() {
    return crypto.getRandomValues(new BigUint64Array(1))[0];
  },
  // @ts-expect-error Not defined in WIT
  insecureRandom() {
    if (insecureRandomValue1 === void 0 || insecureRandomValue2 === void 0) {
      insecureRandomValue1 = random.getRandomU64();
      insecureRandomValue2 = random.getRandomU64();
    }
    return [insecureRandomValue1, insecureRandomValue2];
  }
};

// node_modules/@bytecodealliance/preview2-shim/dist/browser/sockets.js
var sockets_exports = {};
__export(sockets_exports, {
  InMemoryTcpClient: () => InMemoryTcpClient,
  InMemoryTcpSockets: () => InMemoryTcpSockets,
  InMemoryUdpClient: () => InMemoryUdpClient,
  InMemoryUdpSockets: () => InMemoryUdpSockets,
  instanceNetwork: () => instanceNetwork,
  ipNameLookup: () => ipNameLookup,
  network: () => network,
  tcp: () => tcp,
  tcpCreateSocket: () => tcpCreateSocket,
  udp: () => udp,
  udpCreateSocket: () => udpCreateSocket
});

// node_modules/@bytecodealliance/preview2-shim/dist/browser/in-memory-sockets.js
var symbolDispose4 = Symbol.dispose || Symbol.for("dispose");
var unsupported = () => {
  throw "not-supported";
};
function addressKey(address) {
  return `${address.tag}:${address.val.address.join(".")}:${address.val.port}`;
}
function takeBytes(chunks, length) {
  const chunk = chunks.shift();
  if (!chunk) {
    throw { tag: "closed" };
  }
  const requested = checkedU64AsNumber(length, "length");
  if (chunk.byteLength <= requested) {
    return chunk;
  }
  chunks.unshift(chunk.subarray(requested));
  return chunk.subarray(0, requested);
}
var InMemoryTcpClient = class {
  #toServer = [];
  #fromServer = [];
  write(bytes) {
    this.#toServer.push(new Uint8Array(bytes));
  }
  read(length = 4096n) {
    return takeBytes(this.#fromServer, length);
  }
  _serverStreams() {
    return [
      inputStreamCreate({ blockingRead: (length) => takeBytes(this.#toServer, length) }),
      outputStreamCreate({
        write: (bytes) => this.#fromServer.push(new Uint8Array(bytes))
      })
    ];
  }
};
var InMemoryTcpSockets = class {
  #pending = /* @__PURE__ */ new Map();
  tcp;
  tcpCreateSocket;
  constructor() {
    const pending = this.#pending;
    class TcpSocket2 {
      #family;
      #localAddress;
      #remoteAddress;
      #listening = false;
      constructor(family) {
        this.#family = family;
      }
      startBind(_network, localAddress) {
        this.#localAddress = localAddress;
      }
      finishBind() {
        if (!this.#localAddress) {
          throw "not-in-progress";
        }
      }
      startConnect = unsupported;
      finishConnect = unsupported;
      startListen() {
        if (!this.#localAddress) {
          throw "invalid-state";
        }
      }
      finishListen() {
        this.#listening = true;
      }
      accept() {
        if (!this.#listening || !this.#localAddress) {
          throw "invalid-state";
        }
        const client = pending.get(addressKey(this.#localAddress))?.shift();
        if (!client) {
          throw "would-block";
        }
        const connected = new TcpSocket2(this.#family);
        connected.#localAddress = this.#localAddress;
        const [input, output] = client._serverStreams();
        return [connected, input, output];
      }
      localAddress() {
        if (!this.#localAddress) {
          throw "invalid-state";
        }
        return this.#localAddress;
      }
      remoteAddress() {
        if (!this.#remoteAddress) {
          throw "invalid-state";
        }
        return this.#remoteAddress;
      }
      isListening() {
        return this.#listening;
      }
      addressFamily() {
        return this.#family;
      }
      setListenBacklogSize = unsupported;
      keepAliveEnabled = unsupported;
      setKeepAliveEnabled = unsupported;
      keepAliveIdleTime = unsupported;
      setKeepAliveIdleTime = unsupported;
      keepAliveInterval = unsupported;
      setKeepAliveInterval = unsupported;
      keepAliveCount = unsupported;
      setKeepAliveCount = unsupported;
      hopLimit = unsupported;
      setHopLimit = unsupported;
      receiveBufferSize = unsupported;
      setReceiveBufferSize = unsupported;
      sendBufferSize = unsupported;
      setSendBufferSize = unsupported;
      subscribe() {
        return pollableCreate();
      }
      shutdown() {
      }
      [symbolDispose4]() {
      }
    }
    this.tcp = { TcpSocket: TcpSocket2 };
    this.tcpCreateSocket = {
      createTcpSocket: (family) => new TcpSocket2(family)
    };
  }
  connect(serverAddress) {
    const client = new InMemoryTcpClient();
    const key = addressKey(serverAddress);
    const clients = this.#pending.get(key);
    if (clients) {
      clients.push(client);
    } else {
      this.#pending.set(key, [client]);
    }
    return client;
  }
};
var InMemoryUdpClient = class {
  #send;
  #received = [];
  constructor(send) {
    this.#send = send;
  }
  send(bytes, serverAddress) {
    this.#send(bytes, serverAddress);
  }
  read() {
    return takeBytes(this.#received, 65535n);
  }
  _receive(bytes) {
    this.#received.push(new Uint8Array(bytes));
  }
};
var InMemoryUdpSockets = class {
  #datagrams = /* @__PURE__ */ new Map();
  #clients = /* @__PURE__ */ new Map();
  udp;
  udpCreateSocket;
  constructor() {
    const datagrams = this.#datagrams;
    const clients = this.#clients;
    class IncomingDatagramStream2 {
      queue;
      constructor(queue) {
        this.queue = queue;
      }
      receive(maxResults) {
        return this.queue.splice(0, checkedU64AsNumber(maxResults, "max results"));
      }
      subscribe() {
        return pollableCreate();
      }
      [symbolDispose4]() {
      }
    }
    class OutgoingDatagramStream2 {
      remoteAddress;
      #permit = 0;
      constructor(remoteAddress) {
        this.remoteAddress = remoteAddress;
      }
      checkSend() {
        this.#permit = 1024;
        return 1024n;
      }
      send(outgoing) {
        if (outgoing.length > this.#permit) {
          throw new TypeError("datagram count exceeds the permit returned by checkSend");
        }
        this.#permit -= outgoing.length;
        for (const datagram of outgoing) {
          const destination = datagram.remoteAddress ?? this.remoteAddress;
          if (!destination) {
            throw "invalid-argument";
          }
          const client = clients.get(addressKey(destination));
          if (!client) {
            throw "remote-unreachable";
          }
          client._receive(datagram.data);
        }
        return BigInt(outgoing.length);
      }
      subscribe() {
        return pollableCreate();
      }
      [symbolDispose4]() {
      }
    }
    class UdpSocket2 {
      #family;
      #localAddress;
      #remoteAddress;
      constructor(family) {
        this.#family = family;
      }
      startBind(_network, localAddress) {
        this.#localAddress = localAddress;
      }
      finishBind() {
        if (!this.#localAddress) {
          throw "not-in-progress";
        }
      }
      stream(remoteAddress) {
        if (!this.#localAddress) {
          throw "invalid-state";
        }
        this.#remoteAddress = remoteAddress;
        const key = addressKey(this.#localAddress);
        let queue = datagrams.get(key);
        if (!queue) {
          queue = [];
          datagrams.set(key, queue);
        }
        return [
          new IncomingDatagramStream2(queue),
          new OutgoingDatagramStream2(remoteAddress)
        ];
      }
      localAddress() {
        if (!this.#localAddress) {
          throw "invalid-state";
        }
        return this.#localAddress;
      }
      remoteAddress() {
        return this.#remoteAddress;
      }
      addressFamily() {
        return this.#family;
      }
      unicastHopLimit = unsupported;
      setUnicastHopLimit = unsupported;
      receiveBufferSize = unsupported;
      setReceiveBufferSize = unsupported;
      sendBufferSize = unsupported;
      setSendBufferSize = unsupported;
      subscribe() {
        return pollableCreate();
      }
      [symbolDispose4]() {
      }
    }
    this.udp = {
      IncomingDatagramStream: IncomingDatagramStream2,
      OutgoingDatagramStream: OutgoingDatagramStream2,
      UdpSocket: UdpSocket2
    };
    this.udpCreateSocket = {
      createUdpSocket: (family) => new UdpSocket2(family)
    };
  }
  createClient(localAddress) {
    const client = new InMemoryUdpClient((bytes, serverAddress) => {
      const key = addressKey(serverAddress);
      const queue = this.#datagrams.get(key);
      const incoming = { data: new Uint8Array(bytes), remoteAddress: localAddress };
      if (queue) {
        queue.push(incoming);
      } else {
        this.#datagrams.set(key, [incoming]);
      }
    });
    this.#clients.set(addressKey(localAddress), client);
    return client;
  }
};

// node_modules/@bytecodealliance/preview2-shim/dist/browser/sockets.js
var unsupported2 = () => {
  throw "not-supported";
};
var Network = class {
};
var defaultNetwork = new Network();
var instanceNetwork = {
  instanceNetwork: () => defaultNetwork
};
var network = { Network };
var ResolveAddressStream = class {
  resolveNextAddress = unsupported2;
  subscribe = unsupported2;
};
var ipNameLookup = {
  ResolveAddressStream,
  resolveAddresses: unsupported2
};
var TcpSocket = class {
  startBind = unsupported2;
  finishBind = unsupported2;
  startConnect = unsupported2;
  finishConnect = unsupported2;
  startListen = unsupported2;
  finishListen = unsupported2;
  accept = unsupported2;
  localAddress = unsupported2;
  remoteAddress = unsupported2;
  isListening = unsupported2;
  addressFamily = unsupported2;
  setListenBacklogSize = unsupported2;
  keepAliveEnabled = unsupported2;
  setKeepAliveEnabled = unsupported2;
  keepAliveIdleTime = unsupported2;
  setKeepAliveIdleTime = unsupported2;
  keepAliveInterval = unsupported2;
  setKeepAliveInterval = unsupported2;
  keepAliveCount = unsupported2;
  setKeepAliveCount = unsupported2;
  hopLimit = unsupported2;
  setHopLimit = unsupported2;
  receiveBufferSize = unsupported2;
  setReceiveBufferSize = unsupported2;
  sendBufferSize = unsupported2;
  setSendBufferSize = unsupported2;
  subscribe = unsupported2;
  shutdown = unsupported2;
};
var tcpCreateSocket = {
  createTcpSocket: unsupported2
};
var tcp = { TcpSocket };
var IncomingDatagramStream = class {
  receive = unsupported2;
  subscribe = unsupported2;
};
var OutgoingDatagramStream = class {
  checkSend = unsupported2;
  send = unsupported2;
  subscribe = unsupported2;
};
var UdpSocket = class {
  startBind = unsupported2;
  finishBind = unsupported2;
  stream = unsupported2;
  localAddress = unsupported2;
  remoteAddress = unsupported2;
  addressFamily = unsupported2;
  unicastHopLimit = unsupported2;
  setUnicastHopLimit = unsupported2;
  receiveBufferSize = unsupported2;
  setReceiveBufferSize = unsupported2;
  sendBufferSize = unsupported2;
  setSendBufferSize = unsupported2;
  subscribe = unsupported2;
};
var udpCreateSocket = {
  createUdpSocket: unsupported2
};
var udp = {
  IncomingDatagramStream,
  OutgoingDatagramStream,
  UdpSocket
};

// node_modules/@bytecodealliance/preview2-shim/dist/common/instantiation.js
var WASIShim = class {
  /** Object that confirms to the shim interface for `wasi:cli` */
  #cli;
  /** Object that confirms to the shim interface for `wasi:filesystem` */
  #filesystem;
  /** Object that confirms to the shim interface for `wasi:io` */
  #io;
  /** Object that confirms to the shim interface for `wasi:random` */
  #random;
  /** Object that confirms to the shim interface for `wasi:clocks` */
  #clocks;
  /** Object that confirms to the shim interface for `wasi:sockets` */
  #sockets;
  /** Object that confirms to the shim interface for `wasi:http` */
  #http;
  /** Isolated environment for this instance */
  #environment;
  /**
   * Create a new WASIShim instance.
   *
   * @param config - Configuration options
   */
  constructor(config) {
    const shims = config;
    const sandbox = shims?.sandbox;
    const defaultCli = cli_exports;
    this.#cli = shims?.cli ?? (defaultCli.createCli && (shims?.environment !== void 0 || shims?.arguments !== void 0 || shims?.initialCwd !== void 0 || shims?.stdin !== void 0 || shims?.stdout !== void 0 || shims?.stderr !== void 0) ? defaultCli.createCli({
      environment: shims?.environment,
      arguments: shims?.arguments,
      initialCwd: shims?.initialCwd,
      stdin: shims?.stdin,
      stdout: shims?.stdout,
      stderr: shims?.stderr
    }) : defaultCli);
    const defaultFilesystem = filesystem_exports;
    if (shims?.browserFilesystem && !defaultFilesystem.createFilesystem) {
      throw new TypeError("the selected filesystem does not support browser adapters");
    }
    this.#filesystem = shims?.filesystem ?? defaultFilesystem;
    if (shims?.filesystem && sandbox?.preopens !== void 0) {
      if (!shims.filesystem.createPreopens) {
        throw new TypeError("an application-provided filesystem must implement createPreopens to use sandbox.preopens");
      }
      this.#filesystem = {
        types: shims.filesystem.types,
        preopens: shims.filesystem.createPreopens(sandbox.preopens),
        dispose: shims.filesystem.dispose?.bind(shims.filesystem)
      };
    } else if (shims?.browserFilesystem) {
      this.#filesystem = defaultFilesystem.createFilesystem({
        adapter: shims.browserFilesystem.adapter,
        preopens: sandbox?.preopens ?? shims.browserFilesystem.preopens
      });
    } else if (sandbox?.preopens !== void 0) {
      if (!defaultFilesystem.createFilesystem) {
        throw new TypeError("the selected filesystem cannot create isolated preopens");
      }
      this.#filesystem = defaultFilesystem.createFilesystem({
        preopens: sandbox.preopens
      });
    }
    this.#io = shims?.io ?? io_exports;
    this.#random = shims?.random ?? random_exports;
    this.#clocks = shims?.clocks ?? clocks_exports;
    const defaultSockets = sockets_exports;
    this.#sockets = shims?.sockets ?? (defaultSockets.createSockets ? defaultSockets.createSockets({
      enableNetwork: shims?.sandbox?.enableNetwork
    }) : defaultSockets);
    if (shims?.tcpSockets) {
      this.#sockets = {
        ...this.#sockets,
        tcp: shims.tcpSockets.tcp,
        tcpCreateSocket: shims.tcpSockets.tcpCreateSocket
      };
    }
    if (shims?.udpSockets) {
      this.#sockets = {
        ...this.#sockets,
        udp: shims.udpSockets.udp,
        udpCreateSocket: shims.udpSockets.udpCreateSocket
      };
    }
    this.#http = shims?.http ?? http_exports;
    if (sandbox?.enableNetwork === false) {
      this.#http = {
        ...this.#http,
        outgoingHandler: {
          ...this.#http.outgoingHandler,
          handle() {
            throw "access-denied";
          }
        }
      };
    }
    if (shims?.incomingHandler) {
      if (!this.#http.createIncomingHandler) {
        throw new TypeError("the selected HTTP shim does not support Web incoming handlers");
      }
      this.#http = {
        ...this.#http,
        incomingHandler: this.#http.createIncomingHandler(shims.incomingHandler)
      };
    }
    if (sandbox?.env !== void 0 || sandbox?.args !== void 0) {
      this.#environment = createIsolatedEnvironment(sandbox?.env, sandbox?.args, this.#cli);
    }
  }
  /**
   * Generate an import object for the shim that can be used with
   * functions like `instantiate` that are exposed from a transpiled
   * WebAssembly component.
   *
   * @param opts - options for import object generation
   * @returns WASIImportObject
   */
  getImportObject(opts) {
    const versionSuffix = opts?.asVersion ? `@${opts.asVersion}` : "";
    const obj = {};
    obj[`wasi:cli/environment${versionSuffix}`] = this.#environment ?? this.#cli.environment;
    obj[`wasi:cli/exit${versionSuffix}`] = this.#cli.exit;
    obj[`wasi:cli/stderr${versionSuffix}`] = this.#cli.stderr;
    obj[`wasi:cli/stdin${versionSuffix}`] = this.#cli.stdin;
    obj[`wasi:cli/stdout${versionSuffix}`] = this.#cli.stdout;
    obj[`wasi:cli/terminal-input${versionSuffix}`] = this.#cli.terminalInput;
    obj[`wasi:cli/terminal-output${versionSuffix}`] = this.#cli.terminalOutput;
    obj[`wasi:cli/terminal-stderr${versionSuffix}`] = this.#cli.terminalStderr;
    obj[`wasi:cli/terminal-stdin${versionSuffix}`] = this.#cli.terminalStdin;
    obj[`wasi:cli/terminal-stdout${versionSuffix}`] = this.#cli.terminalStdout;
    obj[`wasi:sockets/instance-network${versionSuffix}`] = this.#sockets.instanceNetwork;
    obj[`wasi:sockets/ip-name-lookup${versionSuffix}`] = this.#sockets.ipNameLookup;
    obj[`wasi:sockets/network${versionSuffix}`] = this.#sockets.network;
    obj[`wasi:sockets/tcp${versionSuffix}`] = this.#sockets.tcp;
    obj[`wasi:sockets/tcp-create-socket${versionSuffix}`] = this.#sockets.tcpCreateSocket;
    obj[`wasi:sockets/udp${versionSuffix}`] = this.#sockets.udp;
    obj[`wasi:sockets/udp-create-socket${versionSuffix}`] = this.#sockets.udpCreateSocket;
    obj[`wasi:filesystem/preopens${versionSuffix}`] = this.#filesystem.preopens;
    obj[`wasi:filesystem/types${versionSuffix}`] = this.#filesystem.types;
    obj[`wasi:io/error${versionSuffix}`] = this.#io.error;
    obj[`wasi:io/poll${versionSuffix}`] = this.#io.poll;
    obj[`wasi:io/streams${versionSuffix}`] = this.#io.streams;
    obj[`wasi:random/random${versionSuffix}`] = this.#random.random;
    obj[`wasi:random/insecure${versionSuffix}`] = this.#random.insecure;
    obj[`wasi:random/insecure-seed${versionSuffix}`] = this.#random.insecureSeed;
    obj[`wasi:clocks/monotonic-clock${versionSuffix}`] = this.#clocks.monotonicClock;
    obj[`wasi:clocks/wall-clock${versionSuffix}`] = this.#clocks.wallClock;
    obj[`wasi:http/types${versionSuffix}`] = this.#http.types;
    obj[`wasi:http/incoming-handler${versionSuffix}`] = this.#http.incomingHandler;
    obj[`wasi:http/outgoing-handler${versionSuffix}`] = this.#http.outgoingHandler;
    return obj;
  }
};
function createIsolatedEnvironment(env, args, baseCli) {
  const envEntries = env ? Object.entries(env) : null;
  const argsArray = args || null;
  return {
    ...baseCli.environment,
    getEnvironment() {
      return envEntries ?? baseCli.environment.getEnvironment();
    },
    getArguments() {
      return argsArray ?? baseCli.environment.getArguments();
    },
    initialCwd() {
      return baseCli.environment.initialCwd();
    }
  };
}

// wasi-entry.js
function wasiImports() {
  return new WASIShim({
    sandbox: { preopens: {}, env: {}, args: ["bench"], enableNetwork: false }
  }).getImportObject();
}
export {
  wasiImports
};
