import os.path
import setuptools

root = os.path.abspath(os.path.dirname(__file__))
with open(os.path.join(root, 'README.md'), encoding='utf-8') as f:
    long_description = f.read()

setuptools.setup(
    name='pymuta',
    version='0.4.6',
    url='',
    license='MIT',
    author='mohanson',
    author_email='mohanson@outlook.com',
    description='',
    long_description=long_description,
    long_description_content_type='text/markdown',
    install_requires=[
        'eth-abi==1.3.0',
        'eth-hash[pycryptodome]==0.2.0',
        'eth-keys==0.2.1',
        'eth-typing==2.1.0',
        'eth-utils==1.4.1',
        'protobuf==3.7.1',
    ],
    packages=['pymuta'],
)
