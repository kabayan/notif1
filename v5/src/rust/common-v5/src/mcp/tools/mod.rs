//! MCPツール実装

use std::sync::Arc;
pub mod send;
pub mod draw;
pub mod status;
pub mod devices;

use serde_json::{json, Value};

/// ツールリストを返す
pub async fn list() -> Result<Value, super::JsonRpcError> {
    Ok(json!({
        "tools": [
            {
                "name": "send",
                "description": "Bluetoothディスプレイデバイスにテキストメッセージを送信します。絵文字対応、自動折り返し機能付き。v1 API /send と同等の機能を提供。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "送信するテキスト。絵文字(😊など)と改行(\\n)に対応。32x32グリッドに自動折り返し。"
                        },
                        "device": {
                            "type": "integer",
                            "description": "デバイス番号 (1-9)。省略時は全デバイスに送信。",
                            "minimum": 1,
                            "maximum": 9
                        },
                        "bgcolor": {
                            "type": "string",
                            "description": "背景色。140色の名前(black,red,blue等)、HEX(#FF0000)、RGB(255,0,0)形式対応。",
                            "default": "black",
                            "examples": ["black", "red", "#FF0000", "255,0,0"]
                        },
                        "color": {
                            "type": "string",
                            "description": "文字色。140色の名前、HEX、RGB形式対応。",
                            "default": "white",
                            "examples": ["white", "yellow", "#FFFF00", "255,255,0"]
                        },
                        "size": {
                            "type": "integer",
                            "description": "フォントサイズ (1:小 2:中 3:大 4:特大)",
                            "default": 3,
                            "minimum": 1,
                            "maximum": 4
                        }
                    },
                    "required": ["text"]
                }
            },
            {
                "name": "draw",
                "description": "Bluetoothディスプレイの指定領域に背景色とテキストを描画します。複数領域の同時描画が可能。v2 API /api/draw と同等の機能を提供。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "regions": {
                            "type": "array",
                            "description": "描画する領域の配列。各領域は独立して背景色とテキストを持つ。",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "coords": {
                                        "type": "string",
                                        "description": "領域座標 'row1,col1,row2,col2' 形式 (0-31の範囲)",
                                        "pattern": "^\\d+,\\d+,\\d+,\\d+$",
                                        "examples": ["0,0,31,31", "0,0,15,15", "16,16,31,31"]
                                    },
                                    "bg": {
                                        "type": "string",
                                        "description": "領域の背景色。140色の名前、HEX、RGB形式対応。",
                                        "examples": ["red", "blue", "#00FF00"]
                                    },
                                    "text": {
                                        "type": "string",
                                        "description": "表示するテキスト。絵文字対応。"
                                    },
                                    "tc": {
                                        "type": "string",
                                        "description": "テキスト色。140色の名前、HEX、RGB形式対応。",
                                        "default": "white"
                                    },
                                    "fs": {
                                        "type": "integer",
                                        "description": "フォントサイズ (1-4)",
                                        "default": 2,
                                        "minimum": 1,
                                        "maximum": 4
                                    }
                                },
                                "required": ["text"]
                            }
                        },
                        "device": {
                            "type": "integer",
                            "description": "デバイス番号 (1-9)",
                            "default": 1,
                            "minimum": 1,
                            "maximum": 9
                        },
                        "overwrite": {
                            "type": "boolean",
                            "description": "true: 既存表示を保持して追加描画、false: 画面をクリアしてから描画",
                            "default": false
                        }
                    },
                    "required": ["regions"]
                }
            },
            {
                "name": "status",
                "description": "接続中のBluetoothデバイスの状態を取得します。デバイス番号、ID、接続状態、バッテリー残量などの詳細情報を返します。",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "devices.list",
                "description": "利用可能なBluetoothデバイスの一覧を取得します。接続済み・未接続の両方のデバイス情報を返します。",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "devices.connect",
                "description": "指定したBluetoothデバイスに接続します。MACアドレスを指定して新規接続を確立。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": {
                            "type": "string",
                            "description": "BluetoothデバイスのMACアドレス (例: XX:XX:XX:XX:XX:XX)",
                            "pattern": "^([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}$"
                        }
                    },
                    "required": ["address"]
                }
            },
            {
                "name": "devices.disconnect",
                "description": "接続中のBluetoothデバイスを切断します。デバイス番号を指定して切断。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "device": {
                            "type": "integer",
                            "description": "切断するデバイスの番号 (1-9)",
                            "minimum": 1,
                            "maximum": 9
                        }
                    },
                    "required": ["device"]
                }
            }
        ]
    }))
}