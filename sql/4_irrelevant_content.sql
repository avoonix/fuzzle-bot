update user set blacklist = json_insert(blacklist, '$[#]', 'irrelevant_content');
