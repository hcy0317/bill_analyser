# -*- coding: utf-8 -*-
"""Manual test for statistics API fixes"""

import requests
from datetime import datetime, timedelta
import json

BASE_URL = "http://127.0.0.1:5000"

def get_token():
    """Get auth token"""
    response = requests.post(f"{BASE_URL}/api/auth/login", json={
        'username': 'admin',
        'password': 'admin123'
    })
    if response.status_code != 200:
        print(f"Login failed: {response.text}")
        return None
    return response.json().get('token')

def test_asset_trends_365_days():
    """Test asset trends API - 365 days should work"""
    token = get_token()
    if not token:
        return False
    
    now = datetime.now()
    start_time = int((now - timedelta(days=364)).timestamp())
    end_time = int(now.timestamp())
    
    print("\n" + "="*60)
    print("Test 1: Asset Trends API - 365 days query")
    print("="*60)
    print(f"Start: {datetime.fromtimestamp(start_time)}")
    print(f"End:   {datetime.fromtimestamp(end_time)}")
    print(f"Span:  364 days")
    
    response = requests.get(
        f"{BASE_URL}/api/v1/transactions/statistics/asset_trends.json",
        params={'startTime': start_time, 'endTime': end_time},
        headers={'Authorization': f'Bearer {token}'}
    )
    
    print(f"Status: {response.status_code}")
    data = response.json()
    print(f"Success: {data.get('success')}")
    if response.status_code == 200:
        print(f"Result days: {len(data.get('result', []))}")
        print("✓ PASS - 365 days query succeeded")
        return True
    else:
        print(f"Error: {data.get('error', data.get('errorMessage'))}")
        print("✗ FAIL - 365 days query failed")
        return False

def test_asset_trends_366_days():
    """Test asset trends API - 366 days should fail"""
    token = get_token()
    if not token:
        return False
    
    now = datetime.now()
    start_time = int((now - timedelta(days=366)).timestamp())
    end_time = int(now.timestamp())
    
    print("\n" + "="*60)
    print("Test 2: Asset Trends API - 366 days query (should fail)")
    print("="*60)
    print(f"Start: {datetime.fromtimestamp(start_time)}")
    print(f"End:   {datetime.fromtimestamp(end_time)}")
    print(f"Span:  366 days")
    
    response = requests.get(
        f"{BASE_URL}/api/v1/transactions/statistics/asset_trends.json",
        params={'startTime': start_time, 'endTime': end_time},
        headers={'Authorization': f'Bearer {token}'}
    )
    
    print(f"Status: {response.status_code}")
    data = response.json()
    print(f"Success: {data.get('success')}")
    if response.status_code == 400:
        print(f"Error: {data.get('error', data.get('errorMessage'))}")
        print("✓ PASS - 366 days query correctly rejected")
        return True
    else:
        print("✗ FAIL - 366 days query should have been rejected")
        return False

def test_categorical_missing_params():
    """Test categorical analysis API - missing params should use defaults"""
    token = get_token()
    if not token:
        return False
    
    print("\n" + "="*60)
    print("Test 3: Categorical Analysis - Missing params (use default)")
    print("="*60)
    
    response = requests.get(
        f"{BASE_URL}/api/v1/transactions/statistics.json",
        params={'useTransactionTimezone': 'false'},
        headers={'Authorization': f'Bearer {token}'}
    )
    
    print(f"Status: {response.status_code}")
    data = response.json()
    print(f"Success: {data.get('success')}")
    
    if response.status_code == 200:
        result = data.get('result', {})
        start_ts = result.get('startTime', 0)
        end_ts = result.get('endTime', 0)
        print(f"Start: {datetime.fromtimestamp(start_ts) if start_ts else 'N/A'}")
        print(f"End:   {datetime.fromtimestamp(end_ts) if end_ts else 'N/A'}")
        print(f"Items: {len(result.get('items', []))}")
        print("✓ PASS - Missing params handled with defaults")
        return True
    else:
        print(f"Error: {data.get('error')}")
        print("✗ FAIL - Missing params caused error")
        return False

def test_trend_missing_params():
    """Test trend analysis API - missing params should use defaults"""
    token = get_token()
    if not token:
        return False
    
    print("\n" + "="*60)
    print("Test 4: Trend Analysis - Missing params (use default)")
    print("="*60)
    
    response = requests.get(
        f"{BASE_URL}/api/v1/transactions/statistics/trends.json",
        params={'useTransactionTimezone': 'false'},
        headers={'Authorization': f'Bearer {token}'}
    )
    
    print(f"Status: {response.status_code}")
    data = response.json()
    print(f"Success: {data.get('success')}")
    
    if response.status_code == 200:
        result = data.get('result', [])
        print(f"Months: {len(result)}")
        if result:
            print(f"First: {result[0].get('year')}-{result[0].get('month'):02d}")
            print(f"Last:  {result[-1].get('year')}-{result[-1].get('month'):02d}")
        print("✓ PASS - Missing params handled with defaults")
        return True
    else:
        print(f"Error: {data.get('error')}")
        print("✗ FAIL - Missing params caused error")
        return False

if __name__ == '__main__':
    print("\n" + "="*60)
    print("Statistics API Fixes - Manual Test")
    print("="*60)
    
    results = []
    results.append(("365 days query", test_asset_trends_365_days()))
    results.append(("366 days reject", test_asset_trends_366_days()))
    results.append(("Categorical default", test_categorical_missing_params()))
    results.append(("Trend default", test_trend_missing_params()))
    
    print("\n" + "="*60)
    print("Test Summary")
    print("="*60)
    for name, passed in results:
        status = "✓ PASS" if passed else "✗ FAIL"
        print(f"{status} - {name}")
    
    passed_count = sum(1 for _, p in results if p)
    total_count = len(results)
    print(f"\nTotal: {passed_count}/{total_count} tests passed")
    
    if passed_count == total_count:
        print("\n🎉 All tests passed!")
    else:
        print(f"\n⚠️  {total_count - passed_count} test(s) failed")
