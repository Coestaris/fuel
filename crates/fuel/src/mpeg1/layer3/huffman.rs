#[derive(Copy, Clone)]
pub struct BigHuffmanTableNodeValue {
    pub x: u8,
    pub y: u8,
}

struct BigHuffmanTableNode {
    data: Option<BigHuffmanTableNodeValue>,
    tr0: Option<usize>,
    tr1: Option<usize>,
}
pub struct BigHuffmanTable {
    nodes: Vec<BigHuffmanTableNode>,
    pub linbits: u8,
    root: usize,
}

pub struct BigHuffmanDecoder<'a> {
    table: &'a BigHuffmanTable,
    current: usize,
}

pub enum BigHuffmanDecoderResult {
    Valid(BigHuffmanTableNodeValue),
    InvalidCode,
    NeedMoreBits,
}

impl BigHuffmanDecoder<'_> {
    pub fn new(table: &BigHuffmanTable) -> BigHuffmanDecoder {
        BigHuffmanDecoder {
            table,
            current: table.root,
        }
    }

    fn reset(&mut self) {
        self.current = self.table.root;
    }

    pub fn feed(&mut self, bit: bool) -> BigHuffmanDecoderResult {
        let Some(node) = self.table.nodes.get(self.current) else {
            self.reset();
            return BigHuffmanDecoderResult::InvalidCode;
        };

        let next = if bit { node.tr1 } else { node.tr0 };

        let Some(next) = next else {
            self.reset();
            return BigHuffmanDecoderResult::InvalidCode;
        };

        let Some(next_node) = self.table.nodes.get(next) else {
            self.reset();
            return BigHuffmanDecoderResult::InvalidCode;
        };

        self.current = next;

        if let Some(data) = next_node.data {
            self.reset();
            BigHuffmanDecoderResult::Valid(data)
        } else {
            BigHuffmanDecoderResult::NeedMoreBits
        }
    }
}

#[derive(Copy, Clone)]
pub struct Count1HuffmanTableNodeValue {
    x: u8,
    y: u8,
    z: u8,
    w: u8,
}

struct Count1HuffmanTableNode {
    data: Option<Count1HuffmanTableNodeValue>,
    tr0: Option<usize>,
    tr1: Option<usize>,
}
pub struct Count1HuffmanTable {
    nodes: Vec<Count1HuffmanTableNode>,
    linbits: u8,
    root: usize,
}

struct Count1HuffmanDecoder<'a> {
    table: &'a Count1HuffmanTable,
    current: usize,
}

pub enum Count1HuffmanDecoderResult {
    Valid(Count1HuffmanTableNodeValue),
    InvalidCode,
    NeedMoreBits,
}

impl Count1HuffmanDecoder<'_> {
    pub fn new(table: &Count1HuffmanTable) -> Count1HuffmanDecoder {
        Count1HuffmanDecoder {
            table,
            current: table.root,
        }
    }

    fn reset(&mut self) {
        self.current = self.table.root;
    }

    pub fn feed(&mut self, bit: bool) -> Count1HuffmanDecoderResult {
        let Some(node) = self.table.nodes.get(self.current) else {
            self.reset();
            return Count1HuffmanDecoderResult::InvalidCode;
        };

        let next = if bit { node.tr1 } else { node.tr0 };

        let Some(next) = next else {
            self.reset();
            return Count1HuffmanDecoderResult::InvalidCode;
        };

        let Some(next_node) = self.table.nodes.get(next) else {
            self.reset();
            return Count1HuffmanDecoderResult::InvalidCode;
        };

        self.current = next;

        if let Some(data) = next_node.data {
            self.reset();
            Count1HuffmanDecoderResult::Valid(data)
        } else {
            Count1HuffmanDecoderResult::NeedMoreBits
        }
    }
}
