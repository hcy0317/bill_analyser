// Jest runs frontend unit tests in Node. Some production modules read the
// browser namespace while their classes are being defined, before a test can
// install a focused component mock.
if (!Reflect.has(globalThis, 'window')) {
    Object.defineProperty(globalThis, 'window', {
        configurable: true,
        value: {},
        writable: true
    });
}
