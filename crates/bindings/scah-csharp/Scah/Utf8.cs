using System.Buffers;
using System.Text;

namespace Scah;

/// <summary>UTF-8 conversions at the C ABI boundary.</summary>
internal static unsafe class Utf8
{
    /// <summary>Strings up to this many UTF-8 bytes are encoded on the stack.</summary>
    internal const int StackLimit = 256;

    private static readonly UTF8Encoding Strict = new(
        encoderShouldEmitUTF8Identifier: false,
        throwOnInvalidBytes: true);

    /// <summary>Decode a view that the library has already checked to be UTF-8.</summary>
    internal static string Decode(byte* data, nuint length) =>
        length == 0 ? string.Empty : Encoding.UTF8.GetString(data, checked((int)length));

    /// <summary>The UTF-8 encoding of <paramref name="value"/>, rejecting lone surrogates.</summary>
    /// <remarks>
    /// Encodes into <paramref name="stack"/> when it fits and otherwise into a
    /// pooled array, which <see cref="Buffer.Dispose"/> returns.
    /// </remarks>
    internal static Buffer Encode(string value, Span<byte> stack)
    {
        ArgumentNullException.ThrowIfNull(value);
        int length = Strict.GetByteCount(value);
        byte[]? rented = null;
        Span<byte> bytes = length <= stack.Length
            ? stack[..length]
            : (rented = ArrayPool<byte>.Shared.Rent(length)).AsSpan(0, length);
        Strict.GetBytes(value, bytes);
        return new Buffer(bytes, rented);
    }

    /// <summary>Encoded bytes, possibly in a pooled array.</summary>
    internal readonly ref struct Buffer(Span<byte> bytes, byte[]? rented)
    {
        public readonly Span<byte> Bytes = bytes;

        public void Dispose()
        {
            if (rented is not null)
            {
                ArrayPool<byte>.Shared.Return(rented);
            }
        }
    }
}
