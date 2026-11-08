use std::ops::Range;
use thiserror::Error;

/// A TeX input mistake: `span` is the byte range to point at, `kind` is the reason.
#[derive(Debug, Clone, PartialEq, Error)]
#[error("{kind}")]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ParseErrorKind {
    #[error("式が入力されていません")]
    Empty,
    #[error("未対応のコマンド {0} です。綴りを確認するか、パレットから選んでください")]
    UnknownCommand(String),
    #[error("使えない文字 '{0}' があります。半角の TeX で入力してください")]
    UnexpectedChar(char),
    #[error("{0} が閉じていません。対応する閉じ括弧を追加してください")]
    Unclosed(String),
    #[error("'{0}' に対応する開き括弧がありません。開き括弧を追加するか、この文字を消してください")]
    UnmatchedClose(String),
    #[error("ここに値が必要です。数や変数を入力してください")]
    ExpectedValue,
    #[error("式の後ろに余分な '{0}' があります。消すか、間に演算子を入れてください")]
    Trailing(String),
    #[error("数値の書き方が正しくありません。小数点は1つにしてください")]
    BadNumber,
    #[error("{0} には引数が必要です。{{}} の中に値を入れてください")]
    MissingArgument(String),
    #[error("この式では変数 {0} は使えません。別の文字に変えてください")]
    ReservedVariable(String),
    #[error("左辺 {0} には対応していません。y=, f(x)=, r= のいずれかで書いてください")]
    UnsupportedLhs(String),
    #[error("媒介変数表示は (x(t), y(t)) の2成分で書いてください")]
    BadTuple,
}
