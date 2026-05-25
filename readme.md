# Fuel

Educational MP3 decoder in Rust.

#### MVP plan
- [ ] MVP 0: Save raw bitstream to a WAV file.
- [ ] MVP 1: frame sync, header parser, bitrate/sample-rate tables, skip ID3v2, decode only MPEG-1 Layer III.
- [ ] MVP 2: side info + bit reservoir + Huffman decode.
- [ ] MVP 3: dequantize/requantize, scalefactors, reordering, stereo processing.
- [ ] MVP 4: antialias + IMDCT + overlap-add.
- [ ] MVP 5: synthesis polyphase filterbank → PCM samples.
- [ ] MVP 6: Tests vs minimp3/mpg123: same input → PCM diff / PSNR / sample count.


#### Useful resources
- [ISO/IEC 11172-3:1993 — MPEG-1 Audio](https://www.iso.org/standard/22412.html). Alternative [link](https://courses.e-ce.uth.gr/CE401/tree_menu/tutorials/MPEG1/MPEG1_3.PDF).

Main specification for MPEG-1 Audio, which includes Layer III (MP3). This is the primary reference for the MP3 decoding process.

- [ISO/IEC 13818-3:1995 — MPEG-2 Audio](https://www.iso.org/standard/26797.html)

Lower sampling frequencies and bitrates, but the core decoding process is similar to MPEG-1 Audio. This can be useful for understanding the differences and extensions in MPEG-2.

- [RFC 3003](https://www.rfc-editor.org/rfc/rfc3003.html)

This RFC describes the MIME type for MP3 audio and provides some basic information about the format. It can be useful for understanding how MP3 files are structured and how they are typically used in web contexts.
  
- [MPEG Layer-3 Bitstream Syntax and Decoding](https://mp3guessenc.sourceforge.io/MPEG%20Layer3%20Bitstream%20Syntax%20and%20Decoding.pdf)

Practical guide to the MP3 bitstream syntax and decoding process. This document provides a more hands-on approach to understanding how MP3 decoding works, which can be helpful for implementation.

- [Rassol Raissi — “The Theory Behind MP3”](https://reynal.etis-lab.fr/docs/audio-sia/tp/tp_mp3/mp3_theory.pdf)

This paper provides a detailed explanation of the theory behind MP3 encoding and decoding, including the psychoacoustic model, quantization, and Huffman coding. It can be useful for understanding the underlying principles of MP3.

- [Davis Pan — “A Tutorial on MPEG/Audio Compression”](https://www.ee.columbia.edu/~dpwe/papers/Pan95-mpeg-ieeemm.pdf)

Classical tutorial on MPEG audio compression, covering the basics of the encoding and decoding process. This can be a good starting point for understanding the overall structure of MP3.

- [ISO dist10 reference source](https://github.com/joncampbell123/iso-dist10)

Reference C implementation of the MPEG-1 Audio Layer III decoder. This can be a valuable resource for understanding the implementation details and for testing your own decoder against a known reference.

- [mpg123 source](https://www.mpg123.org/)

Production-grade MP3 decoder library. This can be used for testing and benchmarking your implementation, as well as for understanding how a high-performance MP3 decoder is structured.

- [minimp3 source](https://github.com/lieff/minimp3)

Minimalistic MP3 decoder library. This can be useful for understanding a simpler implementation of an MP3 decoder, which can be easier to follow and learn from.

- [MAD / libmad](https://www.underbit.com/products/mad/)

Fixed-point MP3 decoder library. This can be useful for understanding how MP3 decoding can be implemented in a fixed-point environment, which is common in embedded systems.

- [Symphonia MP3 decoder](https://docs.rs/symphonia)

Rust implementation of an MP3 decoder. This can be useful for understanding how to implement an MP3 decoder in Rust, and for testing your own implementation against a known reference.
