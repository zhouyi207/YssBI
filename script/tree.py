#!/usr/bin/env python3
"""
文件树生成器 - 遍历指定文件夹并显示文件组织结构
忽略所有 .gitignore 文件中指定的文件和文件夹
"""

import os
import sys
from pathlib import Path
from typing import List, Dict
import fnmatch


class GitignoreRule:
    """单个 .gitignore 规则"""
    
    def __init__(self, pattern: str, base_path: Path):
        self.pattern = pattern
        self.base_path = base_path
        self.negation = pattern.startswith('!')
        
        if self.negation:
            self.pattern = pattern[1:]
    
    def matches(self, path: Path) -> bool:
        """检查路径是否匹配此规则"""
        try:
            # 获取相对于规则基础路径的相对路径
            rel_path = path.relative_to(self.base_path)
        except ValueError:
            return False
        
        name = path.name
        rel_path_str = str(rel_path).replace('\\', '/')
        parts = rel_path.parts
        
        pattern = self.pattern
        
        # 如果模式以 / 开头，只匹配从基础路径开始的路径
        if pattern.startswith('/'):
            pattern = pattern[1:]
            if fnmatch.fnmatch(rel_path_str, pattern):
                return True
            return False
        
        # 检查文件名匹配
        if fnmatch.fnmatch(name, pattern):
            return True
        
        # 检查完整相对路径匹配
        if fnmatch.fnmatch(rel_path_str, pattern):
            return True
        
        # 检查路径中的任何部分是否匹配
        for part in parts:
            if fnmatch.fnmatch(part, pattern):
                return True
        
        # 处理包含 / 的模式
        if '/' in pattern:
            if fnmatch.fnmatch(rel_path_str, pattern):
                return True
            # 尝试匹配路径的任意后缀
            for i in range(len(parts)):
                sub_path = '/'.join(parts[i:])
                if fnmatch.fnmatch(sub_path, pattern):
                    return True
        
        return False


class GitignoreManager:
    """管理多个 .gitignore 文件"""
    
    def __init__(self, root_path: Path):
        self.root_path = root_path
        self.rules_by_dir: Dict[Path, List[GitignoreRule]] = {}
        self._load_gitignore_files()
    
    def _load_gitignore_files(self):
        """递归加载所有 .gitignore 文件"""
        for dirpath, dirnames, filenames in os.walk(self.root_path):
            dir_path = Path(dirpath)
            
            # 检查是否有 .gitignore 文件
            if '.gitignore' in filenames:
                gitignore_path = dir_path / '.gitignore'
                rules = self._parse_gitignore(gitignore_path, dir_path)
                if rules:
                    self.rules_by_dir[dir_path] = rules
            
            # 预先过滤掉应该忽略的目录，避免进入
            dirs_to_remove = []
            for dirname in dirnames:
                dir_full_path = dir_path / dirname
                if self.should_ignore(dir_full_path):
                    dirs_to_remove.append(dirname)
            
            for dirname in dirs_to_remove:
                dirnames.remove(dirname)
    
    def _parse_gitignore(self, gitignore_path: Path, base_path: Path) -> List[GitignoreRule]:
        """解析单个 .gitignore 文件"""
        rules = []
        
        try:
            with open(gitignore_path, 'r', encoding='utf-8') as f:
                for line in f:
                    line = line.strip()
                    
                    # 忽略空行和注释
                    if not line or line.startswith('#'):
                        continue
                    
                    # 移除末尾的斜杠
                    if line.endswith('/'):
                        line = line[:-1]
                    
                    rules.append(GitignoreRule(line, base_path))
        except Exception as e:
            print(f"警告: 无法读取 {gitignore_path}: {e}", file=sys.stderr)
        
        return rules
    
    def should_ignore(self, path: Path) -> bool:
        """检查路径是否应该被忽略"""
        # 始终忽略 .git 目录
        if path.name == '.git':
            return True
        
        # 收集所有适用的规则（从根目录到当前路径）
        applicable_rules = []
        
        current = path
        while True:
            try:
                # 检查当前路径的父目录是否有 .gitignore
                parent = current.parent
                if parent in self.rules_by_dir:
                    applicable_rules.extend(self.rules_by_dir[parent])
                
                if parent == self.root_path or parent == parent.parent:
                    break
                
                current = parent
            except:
                break
        
        # 检查根目录的规则
        if self.root_path in self.rules_by_dir:
            applicable_rules.extend(self.rules_by_dir[self.root_path])
        
        # 应用规则（后面的规则优先级更高）
        should_ignore = False
        for rule in applicable_rules:
            if rule.matches(path):
                should_ignore = not rule.negation
        
        return should_ignore


def generate_tree(path: Path, prefix: str = "", is_last: bool = True, 
                  gitignore: GitignoreManager = None, is_root: bool = False) -> str:
    """
    递归生成文件树结构
    
    Args:
        path: 当前路径
        prefix: 当前行的前缀（用于缩进）
        is_last: 是否是当前层级的最后一个项目
        gitignore: gitignore 管理器
        is_root: 是否是根节点
    
    Returns:
        文件树字符串
    """
    # 检查是否应该忽略（根节点不检查）
    if not is_root and gitignore and gitignore.should_ignore(path):
        return ""
    
    result = []
    
    # 添加当前项目
    if is_root:
        result.append(path.name)
    else:
        connector = "└── " if is_last else "├── "
        result.append(f"{prefix}{connector}{path.name}")
    
    # 如果是目录，递归处理子项目
    if path.is_dir():
        try:
            items = sorted(path.iterdir(), key=lambda x: (not x.is_dir(), x.name.lower()))
            
            # 过滤掉应该忽略的项目
            if gitignore:
                items = [item for item in items if not gitignore.should_ignore(item)]
            
            for i, item in enumerate(items):
                is_last_item = (i == len(items) - 1)
                extension = "    " if is_last else "│   "
                new_prefix = prefix + extension if not is_root else ""
                
                subtree = generate_tree(
                    item, 
                    new_prefix, 
                    is_last_item,
                    gitignore,
                    False
                )
                if subtree:
                    result.append(subtree)
        
        except PermissionError:
            result.append(f"{prefix}    [权限被拒绝]")
    
    return "\n".join(result)


def main():
    """主函数"""
    # 获取目标路径
    if len(sys.argv) > 1:
        target_path = sys.argv[1]
    else:
        target_path = "."
    
    target = Path(target_path).resolve()
    
    if not target.exists():
        print(f"错误: 路径 '{target_path}' 不存在")
        sys.exit(1)
    
    # 加载所有 .gitignore 文件
    print("正在扫描 .gitignore 文件...")
    gitignore = GitignoreManager(target)
    
    if gitignore.rules_by_dir:
        print(f"找到 {len(gitignore.rules_by_dir)} 个 .gitignore 文件:")
        for dir_path in sorted(gitignore.rules_by_dir.keys()):
            try:
                rel_path = dir_path.relative_to(target)
                print(f"  - {rel_path if str(rel_path) != '.' else '(根目录)'}")
            except ValueError:
                print(f"  - {dir_path}")
    else:
        print("未找到 .gitignore 文件")
    
    print(f"\n文件树结构: {target}\n")
    
    # 生成并打印文件树
    tree = generate_tree(target, "", True, gitignore, True)
    print(tree)


if __name__ == "__main__":
    main()
