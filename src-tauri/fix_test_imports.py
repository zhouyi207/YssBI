#!/usr/bin/env python3
"""
批量修复测试文件中的导入错误

主要问题：
1. ValueType 导入路径错误
2. 缺少必要的导入
3. 使用了旧的 API（已移除向后兼容性）
"""

import os
import re
from pathlib import Path

def fix_test_file(filepath):
    """修复单个测试文件"""
    print(f"Processing: {filepath}")
    
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    
    original_content = content
    
    # 1. 修复 ValueType 导入
    # 将 `use yssbi_lib::executor::{value::PinTypeDesc, ValueType};` 
    # 改为 `use yssbi_lib::executor::value::{PinTypeDesc, ValueType};`
    content = re.sub(
        r'use yssbi_lib::executor::\{value::PinTypeDesc, ValueType\};',
        'use yssbi_lib::executor::value::{PinTypeDesc, ValueType};',
        content
    )
    
    # 2. 添加缺失的 ValueType 导入
    # 如果文件中使用了 ValueType 但没有导入
    if 'ValueType::' in content and 'use yssbi_lib::executor::value::ValueType' not in content:
        # 查找是否已经有 value 模块的导入
        if 'use yssbi_lib::executor::value::' in content:
            # 已有 value 导入，添加 ValueType
            content = re.sub(
                r'(use yssbi_lib::executor::value::\{[^}]+)\}',
                r'\1, ValueType}',
                content
            )
        else:
            # 没有 value 导入，添加新的导入行
            # 在第一个 use yssbi_lib 之后添加
            content = re.sub(
                r'(use yssbi_lib::executor[^;]+;)',
                r'\1\nuse yssbi_lib::executor::value::ValueType;',
                content,
                count=1
            )
    
    # 3. 修复 pin 模块导入
    # 将 `use yssbi_lib::executor::{BasePin, GenericInDataPin, ...}` 
    # 改为分开导入
    if 'use yssbi_lib::executor::{' in content and 'BasePin' in content:
        # 提取所有从 executor 导入的项
        match = re.search(r'use yssbi_lib::executor::\{([^}]+)\};', content)
        if match:
            imports = match.group(1)
            # 分离 pin 相关和其他导入
            pin_imports = []
            other_imports = []
            
            for item in imports.split(','):
                item = item.strip()
                if any(x in item for x in ['Pin', 'BasePin']):
                    pin_imports.append(item)
                else:
                    other_imports.append(item)
            
            # 重新组织导入
            new_imports = []
            if pin_imports:
                new_imports.append(f"use yssbi_lib::executor::pin::{{{', '.join(pin_imports)}}};")
            if other_imports:
                new_imports.append(f"use yssbi_lib::executor::{{{', '.join(other_imports)}}};")
            
            # 替换原有导入
            content = re.sub(
                r'use yssbi_lib::executor::\{[^}]+\};',
                '\n'.join(new_imports),
                content,
                count=1
            )
    
    # 4. 修复 node_ordering_tests.rs 中的序列化类型错误
    # 将 inputs[0]["type"] 改为 inputs[0]["pin_type"]
    if 'node_ordering_tests' in filepath:
        content = content.replace('["type"]', '["pin_type"]')
    
    # 5. 检查是否有修改
    if content != original_content:
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(content)
        print(f"  ✓ Fixed: {filepath}")
        return True
    else:
        print(f"  - No changes needed: {filepath}")
        return False

def main():
    """主函数"""
    tests_dir = Path('src-tauri/tests')
    
    if not tests_dir.exists():
        print(f"Error: {tests_dir} does not exist")
        return
    
    # 获取所有 .rs 测试文件
    test_files = list(tests_dir.glob('*.rs'))
    
    print(f"Found {len(test_files)} test files")
    print("=" * 60)
    
    fixed_count = 0
    for test_file in test_files:
        if fix_test_file(test_file):
            fixed_count += 1
    
    print("=" * 60)
    print(f"Fixed {fixed_count} out of {len(test_files)} files")

if __name__ == '__main__':
    main()
