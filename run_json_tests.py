#!/usr/bin/env python3
"""
JSON测试用例运行器
读取test_cases.json并执行所有测试
"""

import json
import subprocess
import sys
from pathlib import Path

def run_test_case(test_case, binary_path):
    """运行单个测试用例"""
    name = test_case['name']
    description = test_case['description']
    input_data = test_case['input']
    expected = test_case['expected_output']
    
    print(f"\n{'='*60}")
    print(f"测试: {name}")
    print(f"描述: {description}")
    print(f"{'='*60}")
    
    try:
        # 运行程序
        result = subprocess.run(
            [binary_path],
            input=input_data,
            capture_output=True,
            text=True,
            timeout=10
        )
        
        actual = result.stdout.strip()
        expected = expected.strip()
        
        # 比较输出
        if actual == expected:
            print("✓ 测试通过")
            return True
        else:
            print("✗ 测试失败")
            print(f"\n预期输出:\n{expected}")
            print(f"\n实际输出:\n{actual}")
            
            # 显示差异
            exp_lines = expected.split('\n')
            act_lines = actual.split('\n')
            print(f"\n差异分析:")
            for i, (exp, act) in enumerate(zip(exp_lines, act_lines)):
                if exp != act:
                    print(f"  行{i+1}: 预期 [{exp}] != 实际 [{act}]")
            
            if len(exp_lines) != len(act_lines):
                print(f"  行数不同: 预期{len(exp_lines)}行, 实际{len(act_lines)}行")
            
            return False
            
    except subprocess.TimeoutExpired:
        print("✗ 测试超时")
        return False
    except Exception as e:
        print(f"✗ 执行错误: {e}")
        return False

def main():
    # 读取测试用例
    test_file = Path('test_cases.json')
    if not test_file.exists():
        print(f"错误: 找不到测试文件 {test_file}")
        sys.exit(1)
    
    with open(test_file, 'r', encoding='utf-8') as f:
        data = json.load(f)
    
    test_cases = data['test_cases']
    metadata = data.get('metadata', {})
    
    print(f"测试用例集: {metadata.get('description', 'N/A')}")
    print(f"版本: {metadata.get('version', 'N/A')}")
    print(f"总计: {len(test_cases)} 个测试用例")
    
    if 'categories' in metadata:
        print("\n类别分布:")
        for cat, count in metadata['categories'].items():
            print(f"  - {cat}: {count}")
    
    # 检查可执行文件
    binary_path = Path('target/debug/gmp_for_llm')
    if not binary_path.exists():
        print(f"\n错误: 找不到可执行文件 {binary_path}")
        print("请先运行: cargo build")
        sys.exit(1)
    
    # 运行测试
    passed = 0
    failed = 0
    failed_tests = []
    
    for i, test_case in enumerate(test_cases, 1):
        print(f"\n进度: {i}/{len(test_cases)}")
        if run_test_case(test_case, str(binary_path)):
            passed += 1
        else:
            failed += 1
            failed_tests.append(test_case['name'])
    
    # 输出统计
    print(f"\n{'='*60}")
    print(f"测试完成")
    print(f"{'='*60}")
    print(f"通过: {passed}/{len(test_cases)}")
    print(f"失败: {failed}/{len(test_cases)}")
    print(f"成功率: {passed/len(test_cases)*100:.1f}%")
    
    if failed_tests:
        print(f"\n失败的测试:")
        for name in failed_tests:
            print(f"  - {name}")
        sys.exit(1)
    else:
        print("\n🎉 所有测试通过!")
        sys.exit(0)

if __name__ == '__main__':
    main()
