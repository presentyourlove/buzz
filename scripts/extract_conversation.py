#!/usr/bin/env python3
"""
對話萃取精準度強化腳本

功能：
- 從 Nostr 事件流中萃取純淨對話內容
- 過濾標題、報告雜訊、活動統計等非核心內容
- 僅保留「建議進化項目」與「審核選項」核心決策資訊
- 支援單元測試驗證過濾精準度
"""

import re
import json
from typing import List, Dict, Any, Optional
from dataclasses import dataclass, asdict
from pathlib import Path


@dataclass
class ExtractedMessage:
    """萃取後的純淨訊息結構"""
    event_id: str
    author: str
    timestamp: str
    content: str
    message_type: str  # "proposal" | "review_option" | "decision" | "noise"
    tags: List[List[str]]


class ConversationExtractor:
    """對話萃取器 - 核心過濾邏輯"""
    
    # 需要過濾的雜訊模式
    NOISE_PATTERNS = [
        r'^#{1,6}\s+',  # Markdown 標題
        r'^\s*[\-\*]\s+',  # 列表項目開頭
        r'活動統計',  # 活動統計關鍵字
        r'詳細脈絡',  # 詳細脈絡關鍵字
        r'對話分析',  # 對話分析關鍵字
        r'雜訊',  # 雜訊關鍵字
        r'^\s*$',  # 空行
        r'^[=\-]{3,}$',  # 分隔線
        r'^>\s*',  # 引用塊
        r'```[\s\S]*?```',  # 代碼塊
    ]
    
    # 核心決策關鍵字（保留）
    CORE_KEYWORDS = [
        '建議進化項目',
        '審核選項',
        '授權',
        '同意',
        '駁回',
        '實作',
        '開工',
        '審查',
        '驗收',
    ]
    
    def __init__(self, strict_mode: bool = True):
        self.strict_mode = strict_mode
        self.compiled_noise = [re.compile(p, re.MULTILINE | re.IGNORECASE) for p in self.NOISE_PATTERNS]
    
    def is_noise(self, content: str) -> bool:
        """判斷內容是否為雜訊"""
        if not content or not content.strip():
            return True
        
        # 檢查是否包含核心關鍵字
        has_core = any(kw in content for kw in self.CORE_KEYWORDS)
        if has_core:
            return False
        
        # 檢查雜訊模式
        for pattern in self.compiled_noise:
            if pattern.search(content):
                return True
        
        # 嚴格模式：內容過短且無核心關鍵字視為雜訊
        if self.strict_mode and len(content.strip()) < 20:
            return True
            
        return False
    
    def classify_message(self, content: str) -> str:
        """分類訊息類型"""
        content_lower = content.lower()
        
        if any(kw in content for kw in ['建議進化項目', '進化建議']):
            return "proposal"
        elif any(kw in content for kw in ['審核選項', '審查選項', '選項']):
            return "review_option"
        elif any(kw in content for kw in ['授權', '同意', '駁回', '實作', '開工']):
            return "decision"
        else:
            return "noise"
    
    def clean_content(self, content: str) -> str:
        """清理內容，移除雜訊片段"""
        cleaned = content
        
        # 移除代碼塊
        cleaned = re.sub(r'```[\s\S]*?```', '', cleaned)
        # 移除引用塊
        cleaned = re.sub(r'^>\s*.*$', '', cleaned, flags=re.MULTILINE)
        # 移除標題行
        cleaned = re.sub(r'^#{1,6}\s+.*$', '', cleaned, flags=re.MULTILINE)
        # 移除分隔線
        cleaned = re.sub(r'^[=\-]{3,}$', '', cleaned, flags=re.MULTILINE)
        # 移除雜訊行（活動統計、詳細脈絡、對話分析等）
        for pattern in self.compiled_noise:
            cleaned = re.sub(pattern, '', cleaned)
        # 移除多餘空行
        cleaned = re.sub(r'\n{3,}', '\n\n', cleaned)
        
        return cleaned.strip()
    
    def extract(self, events: List[Dict[str, Any]]) -> List[ExtractedMessage]:
        """主萃取流程"""
        results = []
        
        for event in events:
            content = event.get('content', '')
            parsed = event.get('parsed', {})
            
            # 清理內容
            cleaned_content = self.clean_content(content)
            
            # 判斷是否為雜訊
            if self.is_noise(cleaned_content):
                msg_type = "noise"
            else:
                msg_type = self.classify_message(cleaned_content)
            
            # 只保留非雜訊，或寬鬆模式下保留所有非空內容
            if msg_type != "noise" or (not self.strict_mode and cleaned_content.strip()):
                extracted = ExtractedMessage(
                    event_id=event.get('id', ''),
                    author=event.get('from', ''),
                    timestamp=event.get('time', ''),
                    content=cleaned_content,
                    message_type=msg_type,
                    tags=event.get('tags', [])
                )
                results.append(extracted)
        
        return results
    
    def to_json(self, messages: List[ExtractedMessage]) -> str:
        """序列化為 JSON"""
        return json.dumps([asdict(m) for m in messages], ensure_ascii=False, indent=2)


def load_events_from_file(filepath: str) -> List[Dict[str, Any]]:
    """從檔案載入事件資料"""
    with open(filepath, 'r', encoding='utf-8') as f:
        data = json.load(f)
    return data if isinstance(data, list) else [data]


def main():
    import argparse
    
    parser = argparse.ArgumentParser(description='對話萃取精準度強化工具')
    parser.add_argument('input', help='輸入事件 JSON 檔案路徑')
    parser.add_argument('-o', '--output', help='輸出檔案路徑（預設 stdout）')
    parser.add_argument('--strict', action='store_true', default=True, help='嚴格過濾模式')
    parser.add_argument('--no-strict', dest='strict', action='store_false', help='寬鬆模式')
    
    args = parser.parse_args()
    
    # 載入事件
    events = load_events_from_file(args.input)
    
    # 執行萃取
    extractor = ConversationExtractor(strict_mode=args.strict)
    extracted = extractor.extract(events)
    
    # 輸出結果
    output_json = extractor.to_json(extracted)
    
    if args.output:
        with open(args.output, 'w', encoding='utf-8') as f:
            f.write(output_json)
        print(f"萃取完成：{len(extracted)} 條核心訊息已寫入 {args.output}")
    else:
        print(output_json)


if __name__ == '__main__':
    main()